//! The hall's models, built into the binary (the `embedded` feature), and
//! the great crypt's own models (the `great-crypt` feature).

macro_rules! model {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!(
                "../../../assets/verse/generated/chamber/",
                $name,
                ".glb"
            ))
            .as_slice(),
        )
    };
}

#[cfg(feature = "great-crypt")]
macro_rules! great_crypt {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!(
                "../../../assets/verse/generated/great_crypt/",
                $name,
                ".glb"
            ))
            .as_slice(),
        )
    };
}

/// Every model in [`crate::MODELS`], by name.
pub(crate) static MODELS: &[(&str, &[u8])] = &[
    model!("crypt_hall"),
    model!("slab_table"),
    model!("cauldron_green"),
    model!("cauldron_red"),
    model!("cauldron_amber"),
    model!("candelabrum_tall"),
    model!("candelabrum_short"),
    model!("floor_candles"),
    model!("ritual_rug"),
    model!("specimen_jar"),
    model!("specimen_jar_bones"),
    model!("alchemy_bench"),
    model!("bone_scatter"),
    model!("cobweb"),
    model!("bookshelf"),
    model!("jar_shelf"),
    model!("writing_desk"),
    model!("lectern"),
    model!("chained_skeleton"),
    model!("hanging_chains"),
    model!("brazier"),
    model!("crate"),
    model!("barrel"),
    model!("iron_cage"),
    model!("sarcophagus"),
];

/// The great crypt's own models (`verse_world::great_crypt::CRYPT_MODELS`),
/// by name. Its props are the hall's, in [`MODELS`].
#[cfg(feature = "great-crypt")]
pub(crate) static GREAT_CRYPT: &[(&str, &[u8])] = &[
    great_crypt!("great_crypt_hall"),
    great_crypt!("summoning_circle"),
    great_crypt!("broken_pillar"),
    great_crypt!("rubble_pile"),
];
