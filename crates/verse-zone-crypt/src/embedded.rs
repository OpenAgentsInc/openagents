//! The hall's models, built into the binary (the `embedded` feature).

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

/// Every model in [`crate::MODELS`], by name.
pub(crate) const MODELS: &[(&str, &[u8])] = &[
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
