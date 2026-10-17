use super::*;
use proptest::prelude::*;

fn profile() -> Profile {
    Profile::new(
        "repository-v1",
        [Host::parse("api.example.com").unwrap()].into(),
    )
}
fn rule() -> Destination {
    Destination {
        host: Host::parse("api.example.com").unwrap(),
        port: 443,
        path_prefix: "/v1/messages".into(),
        methods: [Method::Post].into(),
    }
}
#[test]
fn selectors_do_not_accept_urls_wildcards_ports_or_numeric_ip_spellings() {
    for name in [
        "",
        "localhost",
        "*.example.com",
        "api.example.com.",
        "api.example.com:443",
        "good.example@evil.example",
        "https://api.example.com",
        "api.example.com/evil",
        "127.0.0.1",
        "127.1",
        "0177.0.0.1",
        "0x7f000001",
        "2130706433",
        "１２７.０.０.１",
        "[::1]",
        "::ffff:127.0.0.1",
        "a%2eexample.com",
        "a\0.example.com",
        "a..example.com",
        "_a.example.com",
        "-a.example.com",
        "a-.example.com",
    ] {
        assert!(Host::parse(name).is_err(), "{name:?}");
    }
}
#[test]
fn equivalent_idn_and_case_names_have_one_policy_identity() {
    assert_eq!(
        Host::parse("BÜCHER.example"),
        Host::parse("xn--bcher-kva.example")
    );
    assert_eq!(
        Host::parse("API.EXAMPLE.COM"),
        Host::parse("api.example.com")
    );
    let a = Profile::new("idn-v1", [Host::parse("bücher.example").unwrap()].into());
    let b = Profile::new(
        "idn-v1",
        [Host::parse("xn--bcher-kva.example").unwrap()].into(),
    );
    assert_eq!(a.digest(), b.digest());
    assert!(a.permits("BÜCHER.EXAMPLE", "8.8.8.8".parse().unwrap()));
}
#[test]
fn private_names_ips_and_exempt_credential_destinations_are_separate() {
    for ip in [
        "0.1.2.3",
        "10.1.2.3",
        "100.64.0.1",
        "100.127.255.254",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.1.1",
        "192.0.2.3",
        "198.18.1.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.1.1.1",
        "255.255.255.255",
        "::1",
        "::ffff:169.254.169.254",
        "fc00::1",
        "fe80::1",
        "2001:db8::1",
        "2606:4700:4700::1111",
    ] {
        assert!(!public_ip(ip.parse().unwrap()), "{ip}");
    }
    assert!(public_ip("8.8.8.8".parse().unwrap()));
    assert!(public_ip("::ffff:8.8.8.8".parse().unwrap()));
    for name in [
        "metadata.google.internal",
        "api.localhost",
        "api.local",
        "home.arpa",
        "api.home.arpa",
    ] {
        assert!(
            Profile::new("bad", [Host::parse(name).unwrap()].into())
                .validate()
                .is_err()
        );
    }
    assert!(Profile::new("offline", BTreeSet::new()).validate().is_ok());
}
#[test]
fn a_profile_is_a_complete_version_not_an_alias() {
    let original = profile();
    let frozen = original.clone();
    let mut updated = original.clone();
    updated
        .allow_hosts
        .insert(Host::parse("other.example.com").unwrap());
    assert_ne!(frozen.digest(), updated.digest());
    assert!(!frozen.permits("other.example.com", "8.8.8.8".parse().unwrap()));
    let mut exempt = original.clone();
    exempt
        .tls_exempt_hosts
        .insert(Host::parse("api.example.com").unwrap());
    assert_ne!(original.digest(), exempt.digest());
    exempt
        .tls_exempt_hosts
        .insert(Host::parse("outside.example.com").unwrap());
    assert!(exempt.validate().is_err());
}
#[test]
fn credential_paths_methods_and_ports_do_not_gain_authority_by_normalization() {
    let r = rule();
    assert!(r.matches("API.EXAMPLE.COM", 443, "/v1/messages", "POST"));
    assert!(r.matches("api.example.com", 443, "/v1/messages/next", "POST"));
    for path in [
        "/v1/messages-other",
        "/v1/../v1/messages",
        "/v1/%6dessages",
        "/v1/messages%2fnext",
        "//v1/messages",
        "/v1/messages?redirect=evil",
        "/v1/messages\\next",
        "/v1/messages#x",
    ] {
        assert!(!r.matches("api.example.com", 443, path, "POST"), "{path}");
    }
    assert!(!r.matches("api.example.com", 443, "/v1/messages", "GET"));
    assert!(!r.matches("api.example.com", 443, "/v1/messages", "post"));
    assert!(!r.matches("api.example.com.evil.test", 443, "/v1/messages", "POST"));
}
proptest! {
    #[test]
    fn outside_names_never_match_the_profile(label in "[a-z][a-z0-9]{0,24}") {
        let p=profile();let ip="8.8.8.8".parse().unwrap();
        for name in [format!("api.example.com.{label}.test"),format!("{label}.api.example.com"),
            format!("api-{label}.example.com"),format!("{label}@api.example.com"),format!("*.{label}.example.com")]
        { prop_assert!(!p.permits(&name,ip)); }
    }
    #[test]
    fn no_port_suffix_can_change_a_host_selector(port in any::<u16>()) {
        let host = format!("api.example.com:{port}");
        prop_assert!(!profile().permits(&host,"8.8.8.8".parse().unwrap()));
        if port!=443 {prop_assert!(!rule().matches("api.example.com",port,"/v1/messages","POST"));}
    }
    #[test]
    fn every_private_v4_and_mapped_address_is_denied(b in any::<u8>(),c in any::<u8>(),d in any::<u8>()) {
        let ip=std::net::Ipv4Addr::new(10,b,c,d);
        prop_assert!(!public_ip(IpAddr::V4(ip)));
        prop_assert!(!public_ip(IpAddr::V6(ip.to_ipv6_mapped())));
    }
}

proptest! {
    #[test]
    fn a_profile_cannot_use_other_ports_or_quic(port in any::<u16>(),tcp in any::<bool>(),reserved_dns in any::<bool>()) {
        if ![53,443].contains(&port) {prop_assert!(!gateway_transport(tcp,port,reserved_dns));}
        if !reserved_dns && !tcp {prop_assert!(!gateway_transport(false,port,false));}
        if reserved_dns && port!=53 {prop_assert!(!gateway_transport(tcp,port,true));}
    }
    #[test]
    fn unicode_subdomains_and_ip_spellings_do_not_acquire_host_authority(prefix in "[éаß中]{1,6}",part in any::<u8>()) {
        let p=profile();
        let outside=format!("{prefix}.api.example.com");
        prop_assert!(!p.permits(&outside,"8.8.8.8".parse().unwrap()));
        let ip=format!("0x7f0000{part:02x}");
        prop_assert!(!p.permits(&ip,"8.8.8.8".parse().unwrap()));
    }
}
