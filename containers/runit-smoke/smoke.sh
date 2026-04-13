#!/bin/sh
set -eu

service_dir="/etc/service"
service_link="${service_dir}/tux-daemon"
dbus_name="com.tuxedocomputers.tccd"

log() {
    printf '%s\n' "$*"
}

service_status() {
    sv status "$service_link" 2>/dev/null || true
}

extract_pid() {
    printf '%s' "$1" | sed -n 's/.*(pid \([0-9][0-9]*\)).*/\1/p'
}

wait_for_running_pid() {
    attempts="$1"
    i=0
    while [ "$i" -lt "$attempts" ]; do
        status="$(service_status)"
        pid="$(extract_pid "$status")"
        if printf '%s' "$status" | grep -q '^run:' && [ -n "$pid" ]; then
            printf '%s\n' "$pid"
            return 0
        fi
        sleep 0.1
        i=$((i + 1))
    done
    return 1
}

wait_for_dbus_name() {
    attempts="$1"
    i=0
    while [ "$i" -lt "$attempts" ]; do
        if dbus-send \
            --system \
            --dest=org.freedesktop.DBus \
            --type=method_call \
            --print-reply \
            / org.freedesktop.DBus.ListNames 2>/dev/null | grep -q "\"${dbus_name}\""; then
            return 0
        fi
        sleep 0.1
        i=$((i + 1))
    done
    return 1
}

dump_debug() {
    log "---- debug: sv status ----"
    service_status || true
    log "---- debug: daemon log ----"
    cat /tmp/tux-daemon-smoke.log 2>/dev/null || true
    log "---- debug: dbus names ----"
    dbus-send \
        --system \
        --dest=org.freedesktop.DBus \
        --type=method_call \
        --print-reply \
        / org.freedesktop.DBus.ListNames 2>/dev/null || true
    log "---- end debug ----"
}

cleanup() {
    sv down "$service_link" >/dev/null 2>&1 || true
    if [ -n "${runsvdir_pid:-}" ]; then
        kill "$runsvdir_pid" >/dev/null 2>&1 || true
        wait "$runsvdir_pid" >/dev/null 2>&1 || true
    fi
    if [ -n "${dbus_pid:-}" ]; then
        kill "$dbus_pid" >/dev/null 2>&1 || true
        wait "$dbus_pid" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT INT TERM

mkdir -p "$service_dir" /run/dbus
ln -sfn /etc/sv/tux-daemon "$service_link"

dbus-daemon --system --nofork --nopidfile >/dev/null 2>&1 &
dbus_pid="$!"

runsvdir -P "$service_dir" >/dev/null 2>&1 &
runsvdir_pid="$!"

first_pid="$(wait_for_running_pid 300 || true)"
if [ -z "$first_pid" ]; then
    log "smoke failed: service did not enter running state"
    dump_debug
    exit 1
fi
log "smoke ok: runit started daemon pid=${first_pid}"

if ! wait_for_dbus_name 300; then
    log "smoke failed: daemon did not claim dbus name ${dbus_name}"
    dump_debug
    exit 1
fi
log "smoke ok: daemon claimed dbus name ${dbus_name}"

kill -TERM "$first_pid"

new_pid=""
i=0
while [ "$i" -lt 300 ]; do
    status="$(service_status)"
    pid="$(extract_pid "$status")"
    if printf '%s' "$status" | grep -q '^run:' && [ -n "$pid" ] && [ "$pid" != "$first_pid" ]; then
        new_pid="$pid"
        break
    fi
    sleep 0.1
    i=$((i + 1))
done

if [ -z "$new_pid" ]; then
    log "smoke failed: runit did not restart daemon after signal"
    dump_debug
    exit 1
fi
log "smoke ok: runit restarted daemon pid=${new_pid}"

if ! wait_for_dbus_name 300; then
    log "smoke failed: dbus name ${dbus_name} not present after restart"
    dump_debug
    exit 1
fi
log "smoke success: runit + dbus + mock daemon checks passed"