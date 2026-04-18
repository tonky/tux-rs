FROM rust:1.95.0-bookworm AS builder

WORKDIR /work
COPY . .
RUN cargo build --release -p tux-daemon --no-default-features --features tcc-compat

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends dbus runit \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /work/target/release/tux-daemon /usr/bin/tux-daemon
COPY dist/com.tuxedocomputers.tccd.conf /etc/dbus-1/system.d/com.tuxedocomputers.tccd.conf
COPY containers/runit-smoke/service/run /etc/sv/tux-daemon/run
COPY containers/runit-smoke/service/finish /etc/sv/tux-daemon/finish
COPY containers/runit-smoke/smoke.sh /usr/local/bin/runit-smoke

RUN chmod +x \
    /usr/bin/tux-daemon \
    /etc/sv/tux-daemon/run \
    /etc/sv/tux-daemon/finish \
    /usr/local/bin/runit-smoke

CMD ["/usr/local/bin/runit-smoke"]