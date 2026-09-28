use super::*;

const B: Format = Format::Bip177;
const L: Format = Format::LegacyBtc;

#[test]
fn shows_both_formats_at_the_edges() {
    let cases: [(u64, &str, &str, &str); 6] = [
        (0, "₿0", "0.00000000 BTC", "0 bitcoin"),
        (1, "₿1", "0.00000001 BTC", "1 bitcoin"),
        (12_345, "₿12,345", "0.00012345 BTC", "12,345 bitcoin"),
        (
            99_999_999,
            "₿99,999,999",
            "0.99999999 BTC",
            "99,999,999 bitcoin",
        ),
        (
            100_000_000,
            "₿100,000,000",
            "1.00000000 BTC",
            "100,000,000 bitcoin",
        ),
        (
            MAX_SUPPLY,
            "₿2,100,000,000,000,000",
            "21,000,000.00000000 BTC",
            "2,100,000,000,000,000 bitcoin",
        ),
    ];
    for (amount, bip177, legacy, spoken) in cases {
        assert_eq!(B.show(amount), bip177);
        assert_eq!(L.show(amount), legacy);
        assert_eq!(B.spoken(amount), spoken);
        assert_eq!(L.spoken(amount), legacy);
    }
    // The widest u64 still renders; only parsing refuses it.
    assert_eq!(B.show(u64::MAX), "₿18,446,744,073,709,551,615");
    assert_eq!(L.show(u64::MAX), "184,467,440,737.09551615 BTC");
}

#[test]
fn signs_and_units() {
    assert_eq!(B.show_signed(1_000, true), "+₿1,000");
    assert_eq!(B.show_signed(1_000, false), "-₿1,000");
    assert_eq!(L.show_signed(1_000, true), "+0.00001000 BTC");
    assert_eq!(B.unit(), "₿");
    assert_eq!(L.unit(), "BTC");
    assert!(!B.decimal_entry() && L.decimal_entry());
    assert_eq!(B.other(), L);
    assert_eq!(L.other(), B);
    assert_eq!(Format::default(), B);
}

#[test]
fn ids_round_trip() {
    for format in Format::ALL {
        assert_eq!(Format::from_id(format.id()), Some(format));
    }
    assert_eq!(Format::from_id("sats"), None);
    assert_eq!(Format::from_id(" btc\n"), Some(L));
}

#[test]
fn parses_bip177_integers() {
    let ok = [
        ("1", 1),
        ("1000", 1_000),
        ("1,000", 1_000),
        ("₿1,000", 1_000),
        ("₿ 1 000", 1_000),
        ("1_000 bitcoin", 1_000),
        ("12,345 Bitcoins", 12_345),
        ("99,999,999", 99_999_999),
        ("100000000", 100_000_000),
        ("2,100,000,000,000,000", MAX_SUPPLY),
    ];
    for (text, amount) in ok {
        assert_eq!(B.parse(text), Ok(Some(amount)), "{text}");
    }
    for empty in ["", "  ", "₿", "bitcoin"] {
        assert_eq!(B.parse(empty), Ok(None), "{empty:?}");
    }
    assert_eq!(B.parse("0"), Err(ParseError::Zero));
    assert_eq!(B.parse("₿0"), Err(ParseError::Zero));
    assert_eq!(B.parse("2100000000000001"), Err(ParseError::TooLarge));
    assert_eq!(B.parse("18446744073709551616"), Err(ParseError::TooLarge));
    assert_eq!(
        B.parse("99999999999999999999999"),
        Err(ParseError::TooLarge)
    );
    for bad in ["1.5", "0.0001", "-5", "+5", "ten", "1e3", "12 sats", "0x10"] {
        assert_eq!(B.parse(bad), Err(ParseError::Invalid), "{bad}");
    }
}

#[test]
fn parses_legacy_decimals() {
    let ok = [
        ("0.00000001", 1),
        ("0.00012345", 12_345),
        ("0.00012345 BTC", 12_345),
        ("0.00012345btc", 12_345),
        (".5", 50_000_000),
        ("0.99999999", 99_999_999),
        ("1", 100_000_000),
        ("1.", 100_000_000),
        ("1.0", 100_000_000),
        ("1.000000000", 100_000_000),
        ("21,000,000", MAX_SUPPLY),
        ("21000000.00000000 BTC", MAX_SUPPLY),
    ];
    for (text, amount) in ok {
        assert_eq!(L.parse(text), Ok(Some(amount)), "{text}");
    }
    for empty in ["", "BTC", " btc "] {
        assert_eq!(L.parse(empty), Ok(None), "{empty:?}");
    }
    assert_eq!(L.parse("0"), Err(ParseError::Zero));
    assert_eq!(L.parse("0.00000000"), Err(ParseError::Zero));
    assert_eq!(L.parse("0.000000001"), Err(ParseError::TooPrecise));
    assert_eq!(L.parse("1.123456789"), Err(ParseError::TooPrecise));
    assert_eq!(L.parse("21000000.00000001"), Err(ParseError::TooLarge));
    assert_eq!(L.parse("184467440738"), Err(ParseError::TooLarge));
    assert_eq!(L.parse("99999999999999999999"), Err(ParseError::TooLarge));
    for bad in [".", "1.2.3", "-1", "one", "₿1000", "1,5e2"] {
        assert_eq!(L.parse(bad), Err(ParseError::Invalid), "{bad}");
    }
}

#[test]
fn every_shown_amount_parses_back() {
    for amount in [1, 999, 1_000, 12_345, 99_999_999, 100_000_000, MAX_SUPPLY] {
        for format in Format::ALL {
            assert_eq!(format.parse(&format.show(amount)), Ok(Some(amount)));
        }
    }
}

#[test]
fn msat_rounding() {
    assert_eq!(from_msat_floor(1_999), 1);
    assert_eq!(from_msat_ceil(1_001), 2);
    assert_eq!(from_msat_ceil(1_000), 1);
    assert_eq!(from_msat_ceil(0), 0);
}

#[test]
fn messages_name_the_format() {
    assert!(ParseError::Invalid.message(B).contains('₿'));
    assert!(ParseError::Invalid.message(L).contains("BTC"));
    for format in Format::ALL {
        for error in [
            ParseError::Zero,
            ParseError::Invalid,
            ParseError::TooPrecise,
            ParseError::TooLarge,
        ] {
            assert!(!error.message(format).to_lowercase().contains("sat"));
        }
    }
}
