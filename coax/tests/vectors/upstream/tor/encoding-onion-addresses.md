<a id="rend-spec-v3.txt-6"></a>

# Encoding onion addresses \[ONIONADDRESS\]

The onion address of a hidden service includes its identity public key, a
version field and a basic checksum. All this information is then base32
encoded as shown below:

```text
     onion_address = base32(PUBKEY | CHECKSUM | VERSION) + ".onion"
     CHECKSUM = SHA3_256(".onion checksum" | PUBKEY | VERSION)[:2]

     where:
       - PUBKEY is the 32 bytes ed25519 master pubkey (KP_hs_id) of the hidden service.
       - VERSION is a one byte version field (default value '\x03')
       - ".onion checksum" is a constant string
       - CHECKSUM is truncated to two bytes before inserting it in onion_address

  Here are a few example addresses:

       pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.onion
       sp3k262uwy4r2k3ycr5awluarykdpag6a7y33jxop4cs2lu5uz5sseqd.onion
       xa4r2iadxm55fbnqgwwi5mymqdcofiu3w6rpbtqn7b2dyn7mgwj64jyd.onion
```

> For historical notes and rationales about this encoding,
> see [this discussion thread](https://lists.torproject.org/pipermail/tor-dev/2017-January/011816.html).

