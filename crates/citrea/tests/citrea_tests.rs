use openagents_citrea::{
    erc20_balance_of_data, eoa_address_from_secret, format_address, parse_hex_bytes,
    sign_schnorr, verify_schnorr, xonly_pubkey_from_secret,
};

#[test]
fn sign_and_verify_roundtrip() {
    let secret = parse_hex_bytes::<32>(
        "0000000000000000000000000000000000000000000000000000000000000001",
    )
    .expect("parse secret");
    let message = parse_hex_bytes::<32>(
        "0000000000000000000000000000000000000000000000000000000000010203",
    )
    .expect("parse message");
    let pubkey = xonly_pubkey_from_secret(&secret).expect("pubkey");
    let sig = sign_schnorr(&secret, &message).expect("sign");
    let valid = verify_schnorr(&pubkey, &message, &sig).expect("verify");
    assert!(valid);

    let other_message = parse_hex_bytes::<32>(
        "00000000000000000000000000000000000000000000000000000000deadbeef",
    )
    .expect("parse other message");
    let valid_other = verify_schnorr(&pubkey, &other_message, &sig).expect("verify other");
    assert!(!valid_other);
}

#[test]
fn eoa_address_from_secret_key_one() {
    let secret = parse_hex_bytes::<32>(
        "0000000000000000000000000000000000000000000000000000000000000001",
    )
    .expect("secret");
    let address = eoa_address_from_secret(&secret).expect("address");
    let rendered = format_address(&address);
    assert_eq!(
        rendered,
        "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
    );
}

#[test]
fn erc20_balance_of_encoding() {
    let owner = parse_hex_bytes::<20>("1111111111111111111111111111111111111111")
        .expect("owner");
    let data = erc20_balance_of_data(&owner);
    assert_eq!(
        data,
        "0x70a082310000000000000000000000001111111111111111111111111111111111111111"
    );
}
