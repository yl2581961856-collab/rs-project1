#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_NAME="${0##*/}"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"

MODEL_MANIFEST="${SCRIPT_DIR}/models.manifest.tsv"
OUTPUT_DIR="${PWD}/models"
HTTP_TIMEOUT="1800"

fct_log_info() {
    printf '[INFO] %s\n' "$*" >&2
}

fct_log_warn() {
    printf '[WARN] %s\n' "$*" >&2
}

fct_log_error() {
    printf '[ERROR] %s\n' "$*" >&2
}

fct_usage() {
    cat <<'EOF'
download_models.sh

Download model artifacts and verify sha256 for every file.

Usage:
  download_models.sh [options]

Options:
  --manifest PATH     TSV file with 4 columns:
                      name<TAB>url<TAB>sha256<TAB>relative_output_path
                      Default: scripts/models.manifest.tsv
  --output-dir PATH   Model root output directory. Default: ./models
  --timeout SECONDS   Per-file download timeout. Default: 1800
  -h, --help          Show help and exit

Manifest example:
  qwen14b_gguf<TAB>https://example.com/Qwen2.5-14B-Instruct-Q4_K_M.gguf<TAB><sha256><TAB>llm/Qwen2.5-14B-Instruct-Q4_K_M.gguf
  paraformer_onnx<TAB>https://example.com/model.onnx<TAB><sha256><TAB>asr/paraformer/model.onnx

Notes:
  - Lines starting with # are ignored.
  - Existing files are re-used only if sha256 matches.
  - If HF_TOKEN is set, it is sent as Authorization header.
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
            --manifest)
                MODEL_MANIFEST="${2:-}"
                shift 2
                ;;
            --output-dir)
                OUTPUT_DIR="${2:-}"
                shift 2
                ;;
            --timeout)
                HTTP_TIMEOUT="${2:-}"
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

fct_verify_sha256() {
    local file_path="${1}"
    local expected_sha="${2}"
    local actual_sha

    actual_sha="$(sha256sum "${file_path}" | awk '{print $1}')"
    if [[ "${actual_sha}" != "${expected_sha}" ]]; then
        fct_log_error "SHA256 mismatch: ${file_path}"
        fct_log_error "  expected: ${expected_sha}"
        fct_log_error "  actual:   ${actual_sha}"
        return 1
    fi

    return 0
}

fct_download_one() {
    local model_name="${1}"
    local url="${2}"
    local expected_sha="${3}"
    local relative_path="${4}"
    local target_path="${OUTPUT_DIR}/${relative_path}"
    local target_dir
    local tmp_file

    target_dir="$(dirname "${target_path}")"
    mkdir -p "${target_dir}"

    if [[ -f "${target_path}" ]]; then
        if fct_verify_sha256 "${target_path}" "${expected_sha}"; then
            fct_log_info "Skip ${model_name}: already downloaded and verified"
            return 0
        fi

        fct_log_warn "Existing file hash mismatch, re-downloading: ${target_path}"
        rm -f "${target_path}"
    fi

    tmp_file="${target_path}.part"
    rm -f "${tmp_file}"

    fct_log_info "Downloading ${model_name}"

    if [[ -n "${HF_TOKEN:-}" ]]; then
        curl -L --fail --retry 3 --connect-timeout 30 --max-time "${HTTP_TIMEOUT}" \
            -H "Authorization: Bearer ${HF_TOKEN}" \
            -o "${tmp_file}" "${url}"
    else
        curl -L --fail --retry 3 --connect-timeout 30 --max-time "${HTTP_TIMEOUT}" \
            -o "${tmp_file}" "${url}"
    fi

    fct_verify_sha256 "${tmp_file}" "${expected_sha}"
    mv "${tmp_file}" "${target_path}"
    fct_log_info "Downloaded and verified: ${target_path}"
}

fct_process_manifest() {
    local line_no=0
    local model_name=""
    local url=""
    local expected_sha=""
    local relative_path=""

    if [[ ! -f "${MODEL_MANIFEST}" ]]; then
        fct_log_error "Manifest not found: ${MODEL_MANIFEST}"
        fct_log_error "Create the manifest first or pass --manifest PATH"
        exit 2
    fi

    while IFS=$'\t' read -r model_name url expected_sha relative_path || [[ -n "${model_name}${url}${expected_sha}${relative_path}" ]]; do
        line_no=$((line_no + 1))

        if [[ -z "${model_name}" || "${model_name}" == \#* ]]; then
            continue
        fi

        if [[ -z "${url}" || -z "${expected_sha}" || -z "${relative_path}" ]]; then
            fct_log_error "Invalid manifest line ${line_no}: require 4 tab-separated columns"
            exit 2
        fi

        if [[ ! "${expected_sha}" =~ ^[a-fA-F0-9]{64}$ ]]; then
            fct_log_error "Invalid sha256 at line ${line_no}: ${expected_sha}"
            exit 2
        fi

        fct_download_one "${model_name}" "${url}" "${expected_sha}" "${relative_path}"
    done < "${MODEL_MANIFEST}"
}

fct_main() {
    fct_parse_args "$@"

    fct_require_command "curl"
    fct_require_command "sha256sum"
    fct_require_command "awk"

    mkdir -p "${OUTPUT_DIR}"

    fct_log_info "Manifest: ${MODEL_MANIFEST}"
    fct_log_info "Output directory: ${OUTPUT_DIR}"

    fct_process_manifest

    fct_log_info "All model files downloaded and verified"
}

fct_main "$@"
