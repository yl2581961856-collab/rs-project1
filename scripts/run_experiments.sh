#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_NAME="${0##*/}"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"

RUNNER="${PWD}/bench/run_suite.py"
SERVER_URL="ws://127.0.0.1:9000/ws"
OUTPUT_DIR="${PWD}/bench/results/$(date +%Y%m%d_%H%M%S)"
WARMUP_RUNS="20"
DEFAULT_RUNS="100"

AUTO_START_SERVER="0"
SERVER_CMD="cargo run --release"
SERVER_STARTUP_WAIT="8"
SERVER_PID=""

fct_log_info() {
    printf '[INFO] %s\n' "$*" >&2
}

fct_log_error() {
    printf '[ERROR] %s\n' "$*" >&2
}

fct_usage() {
    cat <<'EOF'
run_experiments.sh

Run warmup and E1-E6 experiments for VoxLane.

Usage:
  run_experiments.sh [options]

Options:
  --runner PATH         Experiment runner script. Default: ./bench/run_suite.py
  --server-url URL      WebSocket endpoint. Default: ws://127.0.0.1:9000/ws
  --output-dir PATH     Result directory. Default: ./bench/results/<timestamp>
  --warmup-runs N       Warmup runs. Default: 20
  --runs N              Default runs for E1-E6. Default: 100
  --auto-start-server   Start server command before experiments
  --server-cmd CMD      Server start command (used with --auto-start-server)
  --server-wait SEC     Wait seconds for server startup. Default: 8
  -h, --help            Show help and exit

Environment overrides:
  E1_RUNS ... E6_RUNS   Per-experiment run count override

Runner contract:
  The runner must support arguments:
    --phase warmup|E1|E2|E3|E4|E5|E6
    --runs <int>
    --server-url <ws_url>
    --output-dir <path>
EOF
}

fct_require_command() {
    local cmd="${1}"
    if ! command -v "${cmd}" >/dev/null 2>&1; then
        fct_log_error "Missing required command: ${cmd}"
        exit 4
    fi
}

fct_parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --runner)
                RUNNER="${2:-}"
                shift 2
                ;;
            --server-url)
                SERVER_URL="${2:-}"
                shift 2
                ;;
            --output-dir)
                OUTPUT_DIR="${2:-}"
                shift 2
                ;;
            --warmup-runs)
                WARMUP_RUNS="${2:-}"
                shift 2
                ;;
            --runs)
                DEFAULT_RUNS="${2:-}"
                shift 2
                ;;
            --auto-start-server)
                AUTO_START_SERVER="1"
                shift
                ;;
            --server-cmd)
                SERVER_CMD="${2:-}"
                shift 2
                ;;
            --server-wait)
                SERVER_STARTUP_WAIT="${2:-}"
                shift 2
                ;;
            -h|--help)
                fct_usage
                exit 0
                ;;
            *)
                fct_log_error "Unknown option: $1"
                fct_usage
                exit 2
                ;;
        esac
    done
}

fct_cleanup() {
    if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
        fct_log_info "Stopping server process: ${SERVER_PID}"
        kill "${SERVER_PID}" >/dev/null 2>&1 || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi
}

fct_start_server_if_needed() {
    if [[ "${AUTO_START_SERVER}" != "1" ]]; then
        return 0
    fi

    fct_log_info "Starting server: ${SERVER_CMD}"
    bash -lc "${SERVER_CMD}" >"${OUTPUT_DIR}/server.log" 2>&1 &
    SERVER_PID="$!"

    fct_log_info "Waiting ${SERVER_STARTUP_WAIT}s for server startup"
    sleep "${SERVER_STARTUP_WAIT}"

    if ! kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
        fct_log_error "Server failed to start. Check ${OUTPUT_DIR}/server.log"
        exit 1
    fi
}

fct_run_phase() {
    local phase="${1}"
    local runs="${2}"
    local log_file="${OUTPUT_DIR}/${phase}.log"

    fct_log_info "Running ${phase} (runs=${runs})"

    python3 "${RUNNER}" \
        --phase "${phase}" \
        --runs "${runs}" \
        --server-url "${SERVER_URL}" \
        --output-dir "${OUTPUT_DIR}" \
        2>&1 | tee "${log_file}"
}

fct_get_phase_runs() {
    local phase="${1}"
    local env_var="${phase}_RUNS"
    local val="${DEFAULT_RUNS}"

    if [[ -n "${!env_var:-}" ]]; then
        val="${!env_var}"
    fi

    printf '%s' "${val}"
}

fct_main() {
    trap fct_cleanup EXIT
    fct_parse_args "$@"

    fct_require_command "python3"
    fct_require_command "tee"

    if [[ ! -f "${RUNNER}" ]]; then
        fct_log_error "Runner script not found: ${RUNNER}"
        exit 2
    fi

    mkdir -p "${OUTPUT_DIR}"

    fct_log_info "Runner: ${RUNNER}"
    fct_log_info "Server URL: ${SERVER_URL}"
    fct_log_info "Output: ${OUTPUT_DIR}"

    fct_start_server_if_needed

    fct_run_phase "warmup" "${WARMUP_RUNS}"
    fct_run_phase "E1" "$(fct_get_phase_runs E1)"
    fct_run_phase "E2" "$(fct_get_phase_runs E2)"
    fct_run_phase "E3" "$(fct_get_phase_runs E3)"
    fct_run_phase "E4" "$(fct_get_phase_runs E4)"
    fct_run_phase "E5" "$(fct_get_phase_runs E5)"
    fct_run_phase "E6" "$(fct_get_phase_runs E6)"

    fct_log_info "All phases completed successfully"
}

fct_main "$@"
