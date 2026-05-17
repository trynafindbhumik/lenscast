#!/bin/bash

set -euo pipefail

# =========================
# AndroidCam Configuration
# =========================

PHONE_IP_FILE="$HOME/.androidcam_ip"
PORT_FILE="$HOME/.androidcam_adb_port"
LOG_FILE="$HOME/.androidcam.log"

VIDEO_DEVICE="/dev/video10"
CAMERA_ID="${CAMERA_ID:-0}"

# =========================
# Quality Presets
# =========================

QUALITY="${1:-medium}"

case "$QUALITY" in
  low)
    CAMERA_SIZE="1280x720"
    CAMERA_FPS="24"
    VIDEO_BITRATE="4M"
    ;;
  high)
    CAMERA_SIZE="1920x1080"
    CAMERA_FPS="30"
    VIDEO_BITRATE="12M"
    ;;
  *)
    CAMERA_SIZE="1280x720"
    CAMERA_FPS="30"
    VIDEO_BITRATE="6M"
    ;;
esac

LOCKFILE="/tmp/androidcam.lock"

# =========================
# Colors
# =========================

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# =========================
# Logging
# =========================

exec > >(tee -a "$LOG_FILE") 2>&1

# =========================
# Lock Handling
# =========================

if [[ -f "$LOCKFILE" ]]; then
  OLD_PID="$(cat "$LOCKFILE")"

  if ps -p "$OLD_PID" > /dev/null 2>&1; then
    echo -e "${RED}AndroidCam already running${NC}"
    exit 1
  else
    echo -e "${YELLOW}Removing stale lock file${NC}"
    rm -f "$LOCKFILE"
  fi
fi

echo $$ > "$LOCKFILE"

# =========================
# Cleanup
# =========================

cleanup() {
  echo -e "\n${YELLOW}Cleaning up...${NC}"

  kill "${KEEPALIVE_PID:-}" 2>/dev/null || true

  timeout 2 adb disconnect >/dev/null 2>&1 || true

  sudo modprobe -r v4l2loopback 2>/dev/null || true

  rm -f "$LOCKFILE"
}

trap cleanup EXIT INT TERM

# =========================
# Dependency Checks
# =========================

for cmd in adb scrcpy modprobe timeout; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo -e "${RED}Missing dependency: $cmd${NC}"
    exit 1
  fi
done

# =========================
# Load Phone IP
# =========================

if [[ -f "$PHONE_IP_FILE" ]]; then
  PHONE_IP="$(cat "$PHONE_IP_FILE")"
  echo -e "${GREEN}Using saved phone IP: $PHONE_IP${NC}"
else
  read -r -p "Enter phone IP: " PHONE_IP
  echo "$PHONE_IP" > "$PHONE_IP_FILE"
fi

# =========================
# ADB Helpers
# =========================

reset_adb() {
  adb start-server >/dev/null 2>&1 || true
}

connect_adb() {
  local port="$1"
  local addr="${PHONE_IP}:${port}"

  reset_adb

  echo -e "${YELLOW}Trying ADB connect to $addr ...${NC}"

  timeout 5 adb connect "$addr" >/dev/null 2>&1 || return 1

  sleep 1

  timeout 5 adb shell echo connected >/dev/null 2>&1 || return 1

  return 0
}

# =========================
# Try Saved Port
# =========================

if [[ -f "$PORT_FILE" ]]; then
  SAVED_PORT="$(cat "$PORT_FILE")"

  if [[ -n "$SAVED_PORT" ]] && connect_adb "$SAVED_PORT"; then
    ADB_PORT="$SAVED_PORT"

    echo -e "${GREEN}Connected using saved port: $ADB_PORT${NC}"
  else
    echo -e "${YELLOW}Saved port failed${NC}"
  fi
fi

# =========================
# Ask For Port If Needed
# =========================

if [[ -z "${ADB_PORT:-}" ]]; then
  while true; do
    read -r -p "Enter Wireless ADB port (from phone): " ADB_PORT

    [[ -z "$ADB_PORT" ]] && continue

    if connect_adb "$ADB_PORT"; then
      echo "$ADB_PORT" > "$PORT_FILE"

      echo -e "${GREEN}Connected and port saved${NC}"

      break
    else
      echo -e "${RED}Connection failed. Check Wireless debugging.${NC}"
    fi
  done
fi

# =========================
# Keep Phone Awake
# =========================

adb shell svc power stayon true

# =========================
# Keepalive
# =========================

(
  while true; do
    timeout 5 adb shell "echo ping >/dev/null" || break
    sleep 20
  done
) &
KEEPALIVE_PID=$!

# =========================
# Webcam Module Setup
# =========================

sudo modprobe -r v4l2loopback 2>/dev/null || true

sudo modprobe v4l2loopback \
  devices=1 \
  video_nr=10 \
  card_label="AndroidCam" \
  exclusive_caps=1

sleep 2

# =========================
# Verify Webcam
# =========================

if [[ ! -e "$VIDEO_DEVICE" ]]; then
  echo -e "${RED}Failed to create virtual webcam${NC}"
  exit 1
fi

echo -e "${GREEN}Virtual webcam ready at $VIDEO_DEVICE${NC}"

# =========================
# Launch Camera Stream
# =========================

echo -e "${BLUE}Launching Android camera...${NC}"

scrcpy \
  --video-source=camera \
  --camera-id="$CAMERA_ID" \
  --camera-size="$CAMERA_SIZE" \
  --camera-fps="$CAMERA_FPS" \
  --video-bit-rate="$VIDEO_BITRATE" \
  --video-codec=h264 \
  --v4l2-sink="$VIDEO_DEVICE" \
  --no-playback
