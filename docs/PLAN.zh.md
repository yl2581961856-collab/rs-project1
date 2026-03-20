# VoxLane 论文与研发总规划 (中文)

## 1. 项目定位

VoxLane 是一个面向边缘部署场景的实时语音 AI 管道中间件，采用 Rust 实现。

核心目标不是更换模型，而是在相同 ASR/LLM/TTS 模型下，通过系统层设计优化实现:

- 更低且更稳定的 tail latency (p95/p99)
- 更快且更可控的打断传播延迟 (Cancel Propagation Delay, CPD)
- 严格的 Turn-Cancel 一致性，避免旧 turn 数据泄漏

一句话核心论点:

> 在三段式语音管道 (ASR -> LLM -> TTS) 中，系统中间件层的调度与一致性语义是影响实时体验的关键变量；VoxLane 在相同模型下显著改善延迟稳定性与打断正确性。

## 2. 三个创新点与论证方向

### 2.1 创新点 1: Turn-Cancel 一致性协议

关注点:

- 单 session 单 active turn
- 新语音 `supersede` 旧 turn
- `cancel` 最高优先级，需传播到 ASR/LLM/TTS 全链路
- 旧 turn 迟到数据必须丢弃

论证重点:

- 不是“可以打断”，而是“打断后零泄漏且可验证”
- 使用状态机单测 + property-based testing + 压力场景验证

### 2.2 创新点 2: IO/计算分离调度

目标架构:

- Fast Lane: Monoio/io_uring，负责 WebSocket、音频帧快路径
- Compute Lane: ASR/LLM/TTS Worker
- Slow Lane: Tokio 负责非实时慢路径逻辑

论证重点:

- 不是“io_uring 更快”
- 而是“在高负载/打断场景下，CPD 与 p99 更稳定”

### 2.3 创新点 3: 模型无关系统层优化

实验设计原则:

- 同模型、同硬件、同输入语料
- 仅对比系统实现差异

论证重点:

- 不是“Rust 天生更快”
- 而是“系统调度与状态机语义带来的可量化收益”

## 3. 论文叙事框架

## 3.1 论文主线

- 问题: 现有实时语音框架在 tail latency 与打断一致性上缺乏保证
- 方法: VoxLane 的一致性协议 + 混合运行时调度
- 结论: 在相同模型下，VoxLane 获得更低的 p95/p99、更低 CPD、更低泄漏风险

### 3.2 每个创新点的证据结构

- 设计: 协议/调度机制定义
- 指标: TTFT/TTFA/CPD/stale leakage
- 对照: VoxLane vs Pipecat
- 消融: Monoio+Tokio vs Tokio only

### 3.3 关键图表清单

- 端到端延迟 CDF 曲线
- CPD 箱线图
- Stale Leakage 柱状图
- 并发退化曲线 (1/5/10/20 session)
- 延迟分解堆叠图
- 消融对比表
- 网络抖动分组图 (E7)

## 4. 系统架构规划

### 4.1 逻辑架构

```text
Client WS <-> Fast Lane (Monoio WS)
                 |
                 | crossbeam-channel bridge
                 v
      Tokio Session State Machine
                 |
         ASR -> LLM -> TTS Workers
```

### 4.2 状态机模型

- 状态: `Listening`, `Thinking`, `Speaking`
- 模型: `handle(Event) -> Vec<Command>`
- Turn 规则:
  - Turn ID 单调递增
  - 新 turn supersede 旧 turn
  - 非 active turn 的事件直接丢弃

### 4.3 Bridge 接口 (已定义)

见 `src/core/bridge.rs`:

- `RawWsMessage { Text, Binary, Close }`
- `OutMessage { Text, Binary, Close }`

### 4.4 流式管道

- ASR: chunk 级流式输入与 partial/final 输出
- LLM: token 级流式输出
- 文本切分器: 将 LLM token 聚合为可合成片段
- TTS: 片段流式输出音频，首帧即发

## 5. 技术栈与基线

### 5.1 模型与组件

- ASR: Paraformer-zh-streaming (sherpa-onnx)
- LLM: Qwen2.5-14B-Instruct Q4_K_M (llama.cpp / llama-cpp-2)
- TTS 主实验: sherpa-onnx TTS (嵌入式)
- TTS 补充实验: CosyVoice (Unix Domain Socket)

### 5.2 运行时

- Monoio 0.2: Fast Lane
- Tokio: Session 与 Worker 管理
- crossbeam-channel: Monoio/Tokio 桥接

### 5.3 Baseline

- Pipecat (Python)
- 要求同模型、同输入、同硬件条件下对比

## 6. 实验设计 (E1-E7)

### 6.0 指标定义

- TTFT: ASR final -> LLM 首 token
- TTFA: ASR final -> TTS 首音频帧
- CPD: cancel 发起 -> 全链路停止确认
- Stale Leakage: 打断后旧 turn 泄漏数据量
- Gap Count: 音频流断续次数

### E1 单 Session 正常对话

- 目标: 基础延迟分布对比
- 对照: VoxLane vs Pipecat
- 指标: TTFT/TTFA/E2E p50/p95/p99

### E2 打断场景 (Barge-in)

- 目标: 验证 cancel 一致性
- 指标: CPD + stale leakage

### E3 并发多 Session

- 目标: 验证 tail latency 稳定性
- 并发度: 1/5/10/20

### E4 长对话稳定性

- 目标: 验证长时间运行的延迟漂移与内存稳定性

### E5 高频连续打断压力测试

- 目标: 验证鲁棒性
- 场景: 每 500ms 触发一次打断，持续 2 分钟

### E6 消融与延迟分解

- 目标: 定位收益来源
- 对照: Monoio+Tokio vs Tokio only vs Pipecat

### E7 网络抖动实验 (策略 B)

- 目标: 验证弱网条件下实时性鲁棒性
- 方式: 单机 loopback + `tc netem` 注入抖动
- 档位:
  - 低: `delay 10ms 5ms`
  - 中: `delay 40ms 20ms`
  - 高: `delay 80ms 40ms loss 2%`

备注:

- 当前 `scripts/run_experiments.sh` 已支持 warmup + E1-E6
- E7 将在实验脚本下一轮更新中加入

## 7. 资源与部署策略

### 7.1 开发策略

- 本地开发机用于功能打通与调试
- 基线代码准备完整后，再上云跑完整实验

### 7.2 云端实验策略

- 单机 A800 租赁
- 一次性执行 E1-E7，控制成本

### 7.3 模型与实验脚本

- `scripts/download_models.sh`: 模型下载 + sha256 校验
- `scripts/run_experiments.sh`: warmup + E1-E6 (E7 待补)

## 8. 分工与任务清单

### 8.1 WS/底层任务 (Y 系列)

| 编号 | 任务 | 依赖 | 预估 |
| --- | --- | --- | --- |
| Y1 | Monoio WS 服务端: accept + HTTP 101 upgrade | 无 | 2 天 |
| Y2 | Monoio WS 服务端: read/write split | Y1 | 1 天 |
| Y3 | Monoio WS 服务端: crossbeam bridge 接入 | Y2 | 1 天 |
| Y4 | Monoio WS 服务端: 测试与压测 | Y3 | 1 天 |
| Y5 | sherpa-rs fork: Online Recognizer 封装 | 无 | 2 天 |
| Y6 | A800 环境核验 | 无 | 1 天 |

### 8.2 业务与实验任务 (M 系列)

| 编号 | 任务 | 依赖 | 预估 |
| --- | --- | --- | --- |
| M1 | ASR Worker + sherpa-onnx online 接入 | Y5 | 5 天 |
| M2 | LLM Worker + 文本切分器 | 无 | 5 天 |
| M3 | TTS Worker (sherpa-onnx 嵌入式) | 无 | 3 天 |
| M4 | TTS Worker (CosyVoice UDS) | 无 | 3 天 |
| M5 | Monoio WS 集成与 feature flag | Y3 | 3 天 |
| M6 | Pipecat baseline 搭建与埋点 | 无 | 4 天 |
| M7 | `bench/run_suite.py` 实现 E1-E7 | M1-M6 | 4 天 |
| M8 | `bench/analyze.py` 图表生成 | M7 | 2 天 |
| M9 | `models.manifest.tsv` 填充真实 URL/hash | 无 | 0.5 天 |
| M10 | `run_experiments.sh` 增加 E7 + netem 自动化 | 无 | 0.5 天 |

### 8.3 关键路径

```text
Y1 -> Y2 -> Y3 -> M5
Y5 -> M1
M2/M3/M4/M6 并行
M1/M2/M3/M4/M5/M6 -> M7 -> M8 -> 云端实验
```

## 9. 目标投稿列表

### 9.1 首选目标

- ACM Middleware (CCF-B)

### 9.2 备选目标

- IEEE ICDCS (CCF-B)
- IEEE/ACM IWQoS (CCF-B)
- IEEE RTAS (CCF-B)
- IEEE RTSS (CCF-A)
- IEEE INFOCOM (CCF-A)
- IEEE LCN (CCF-C)

## 10. 当前完成进度 (截至当前仓库状态)

已完成:

- 项目重命名为 VoxLane
- WebSocket 协议类型定义与解析
- Session 命令执行主干
- Bridge 接口定义
- 状态机 bug 修复
- 状态机测试与协议测试通过
- 模型下载脚本与实验调度脚本落地

当前仓库:

- `README.md` 聚焦协议与测试标准
- 本文档用于论文与研发全局规划
