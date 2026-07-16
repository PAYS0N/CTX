//! Content invariants of the embedded cage assets — the properties that
//! fail silently at runtime rather than at build time.

use ctx_cage::CAGE_RESOLV_CONF;

/// Nameserver addresses declared by the stub resolv.conf.
fn nameservers() -> Vec<&'static str> {
    CAGE_RESOLV_CONF
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("nameserver "))
        .map(str::trim)
        .collect()
}

/// An empty (or comment-only) resolv.conf is not inert: the resolver
/// falls back to its `127.0.0.1:53` default, exactly the case that
/// spins. It must name a server explicitly.
#[test]
fn stub_resolv_conf_declares_a_nameserver() {
    assert!(
        !nameservers().is_empty(),
        "cage-resolv.conf must declare a nameserver; an empty file \
         means the resolver defaults to 127.0.0.1:53 (ADR-049)"
    );
}

/// The load-bearing property, and the one a well-meaning edit would
/// break: the cage's netns has loopback up with nothing on :53, so a
/// loopback nameserver is refused *instantly* and retried in a tight
/// loop — a full core burned for the whole session. Any non-loopback
/// address fails slowly instead, which is all that is required; the
/// address is never actually reached (the cage is offline).
#[test]
fn stub_resolv_conf_nameservers_are_not_loopback() {
    for ns in nameservers() {
        assert!(
            !ns.starts_with("127.") && ns != "::1" && !ns.starts_with("[::1]"),
            "cage-resolv.conf nameserver {ns} is loopback: the cage \
             refuses it instantly and the resolver spins (ADR-049)"
        );
    }
}
