//! The Archdruid's bar: four rows of twelve slots, what each holds, and
//! each ability's icon and tooltip sentence (`docs/verse/druid-demo.md`,
//! The action bar).
//!
//! Row 1 opens with Meteor Swarm and the Thunderbolt on keys 1 and 2, then
//! Wild Shape and the druid's features; row 2 holds the cantrips and levels 1
//! and 2, row 3 levels 2 to 7, and row 4 levels 8 and 9, Speak with
//! Animals, the chosen land's six spells on keys 5 to 0, and Wild
//! Resurgence and Long Rest on - and =. In a beast's shape, or
//! Shapechange's dragon, its attacks take row 2's first slots, and the
//! other rows still cast, as Beast Spells allows. The tooltip sentences are our own
//! summaries of SRD 5.2.1; the numbers on a card come from [`Spell::def`].

use super::kit::{Land, Spell};
use super::shape::Form;

/// Rows on the bar.
pub const ROWS: usize = 4;
/// Slots in a row.
pub const COLUMNS: usize = 12;
/// Slots on the bar.
pub const COUNT: usize = ROWS * COLUMNS;

/// What a slot holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Spell(Spell),
    /// The chosen land's spell at this index, 0 to 5.
    Land(usize),
    Empty,
}

/// The bar, row by row, in the druid's own shape.
pub const LAYOUT: [[Entry; COLUMNS]; ROWS] = {
    use Entry::{Land as L, Spell as S};
    use Spell as K;
    [
        [
            S(K::MeteorSwarm),
            S(K::Thunderbolt),
            S(K::WildShapeBear),
            S(K::WildShapeWolf),
            S(K::WildShapeEagle),
            S(K::WildShapeSpider),
            S(K::ReturnToForm),
            S(K::WildCompanion),
            S(K::LandsAid),
            S(K::NaturesSanctuary),
            S(K::ChooseLand),
            S(K::NatureMagician),
        ],
        [
            S(K::ProduceFlame),
            S(K::StarryWisp),
            S(K::Shillelagh),
            S(K::PoisonSpray),
            S(K::Elementalism),
            S(K::Thunderwave),
            S(K::Entangle),
            S(K::FaerieFire),
            S(K::IceKnife),
            S(K::HealingWord),
            S(K::Moonbeam),
            S(K::GustOfWind),
        ],
        [
            S(K::SpikeGrowth),
            S(K::CallLightning),
            S(K::ConjureAnimals),
            S(K::WindWall),
            S(K::IceStorm),
            S(K::WallOfFire),
            S(K::Polymorph),
            S(K::MassCureWounds),
            S(K::Sunbeam),
            S(K::WallOfThorns),
            S(K::FireStorm),
            S(K::ReverseGravity),
        ],
        [
            S(K::Sunburst),
            S(K::StormOfVengeance),
            S(K::Shapechange),
            S(K::SpeakWithAnimals),
            L(0),
            L(1),
            L(2),
            L(3),
            L(4),
            L(5),
            S(K::WildResurgence),
            S(K::LongRest),
        ],
    ]
};

/// The row a beast's attacks replace.
pub const BEAST_ROW: usize = 1;

/// What slot `index` casts in `form`'s shape, if any, with `land` chosen.
#[must_use]
pub fn spell(index: usize, form: Option<Form>, land: Land) -> Option<Spell> {
    let (row, column) = (index / COLUMNS, index % COLUMNS);
    if row == BEAST_ROW
        && let Some(attack) = form.and_then(|f| f.attacks().get(column).copied())
    {
        return Some(attack);
    }
    match *LAYOUT.get(row)?.get(column)? {
        Entry::Spell(spell) => Some(spell),
        Entry::Land(i) => land.spells().get(i).copied(),
        Entry::Empty => None,
    }
}

/// The slot `spell` sits on, if any, in `form`'s shape with `land`.
#[must_use]
pub fn slot_of(spell: Spell, form: Option<Form>, land: Land) -> Option<usize> {
    (0..COUNT).find(|&i| self::spell(i, form, land) == Some(spell))
}

/// An ability's icon sprite and its tooltip's sentence: what it does, in
/// our own words. The card adds its range, dice, save, and area from
/// [`Spell::def`].
#[must_use]
pub const fn info(spell: Spell) -> (&'static str, &'static str) {
    use Spell as S;
    match spell {
        S::WildShapeBear => (
            "wild-shape-bear-icon",
            "Become a Brown Bear, slow and sturdy, with a bite and a claw that knocks dummies down.",
        ),
        S::WildShapeWolf => (
            "wild-shape-wolf-icon",
            "Become a Dire Wolf, half again as fast as you run, whose bite knocks a dummy down.",
        ),
        S::WildShapeEagle => (
            "wild-shape-eagle-icon",
            "Become a Giant Eagle and take to the air: hold Jump to climb and X to descend, and rake with your talons.",
        ),
        S::WildShapeSpider => (
            "wild-shape-spider-icon",
            "Become a Giant Spider, a quarter faster than you run, with its venomous bite and its web.",
        ),
        S::ReturnToForm => (
            "return-to-form-icon",
            "Drop the beast's shape and stand as the druid again.",
        ),
        S::WildCompanion => (
            "wild-companion-icon",
            "Calls a fey owl familiar to circle you and mark targets; for now a labeled burst.",
        ),
        S::LandsAid => (
            "lands-aid-icon",
            "Flowers and thorns burst at the dummy ahead: 4d6 necrotic in a 10-foot radius, half on a Constitution save.",
        ),
        S::NaturesSanctuary => (
            "natures-sanctuary-icon",
            "Raises a cube of spectral trees that gives cover; for now a labeled burst.",
        ),
        S::ChooseLand => (
            "choose-land-icon",
            "Choose the next land, Arid, Polar, Temperate, or Tropical, and its six spells take row 4.",
        ),
        S::NatureMagician => (
            "nature-magician-icon",
            "Turns Wild Shape uses into spell power, which the Grove doesn't track; a labeled burst.",
        ),
        S::WildResurgence => (
            "wild-resurgence-icon",
            "Trades spell power for a Wild Shape use, which the Grove doesn't track; a labeled burst.",
        ),
        S::LongRest => (
            "long-rest-icon",
            "Ends your spells and your beast's shape and stands the dummies back up, healed and home.",
        ),
        S::ProduceFlame => (
            "produce-flame-icon",
            "Hurls the flame in your hand at the dummy ahead: a spell attack for 4d8 + 5 fire.",
        ),
        S::StarryWisp => (
            "starry-wisp-icon",
            "Flings a mote of starlight: a spell attack for 4d8 + 5 radiant that leaves the dummy starlit and unable to hide.",
        ),
        S::Shillelagh => (
            "shillelagh-icon",
            "Your staff glows and strikes the dummy at hand: a spell attack for 2d6 + 5 force.",
        ),
        S::PoisonSpray => (
            "poison-spray-icon",
            "Sprays a puff of poison at a dummy within 30 feet: a spell attack for 4d12 + 5 poison.",
        ),
        S::Elementalism => (
            "elementalism-icon",
            "Stirs a harmless gust, ember, or tremor by the dummy ahead, for show.",
        ),
        S::Thunderwave => (
            "thunderwave-icon",
            "A 15-foot cube of thunder from you deals 2d8 and pushes dummies 10 feet; a Constitution save halves it and holds ground.",
        ),
        S::Entangle => (
            "entangle-icon",
            "Grasping vines fill a 20-foot square at the dummy ahead and root each one failing a Strength save for 3 seconds.",
        ),
        S::FaerieFire => (
            "faerie-fire-icon",
            "Outlines dummies in a 20-foot cube that fail a Dexterity save: they take a fifth more damage for 10 seconds.",
        ),
        S::IceKnife => (
            "ice-knife-icon",
            "Throws a shard of ice for a 1d10 piercing spell attack that bursts for 2d6 cold around it, avoided on a Dexterity save.",
        ),
        S::HealingWord => (
            "healing-word-icon",
            "A green rune heals the dummy ahead for 2d4 + 5, without breaking stride.",
        ),
        S::Moonbeam => (
            "moonbeam-icon",
            "A column of silver light at the dummy ahead burns 2d10 radiant a second, half on a Constitution save; cast again to move it.",
        ),
        S::GustOfWind => (
            "gust-of-wind-icon",
            "A 60-foot line of wind pushes each dummy in it 15 feet away unless it makes a Strength save.",
        ),
        S::SpikeGrowth => (
            "spike-growth-icon",
            "Thorns cover a 20-foot radius at the dummy ahead: each 5 feet a dummy is moved through them costs it 2d4 piercing.",
        ),
        S::CallLightning => (
            "call-lightning-icon",
            "A storm cloud gathers over the dummy ahead and strikes for 3d10 lightning, half on a Dexterity save, every 6 seconds and each time you cast again.",
        ),
        S::ConjureAnimals => (
            "conjure-animals-icon",
            "A spectral pack tears through a 10-foot cube at the dummy ahead: 3d10 slashing unless it makes a Dexterity save.",
        ),
        S::WindWall => (
            "wind-wall-icon",
            "Raises a wall of wind at the dummy ahead: 4d8 bludgeoning, half on a Strength save, and a failed save is thrown upward, as you are if you walk in.",
        ),
        S::IceStorm => (
            "ice-storm-icon",
            "Hail pounds a 20-foot radius at the dummy ahead: 2d10 bludgeoning and 4d6 cold, half on a Dexterity save.",
        ),
        S::WallOfFire => (
            "wall-of-fire-icon",
            "Raises a 60-foot wall of fire across the dummy ahead: 5d8 fire as it rises, half on a Dexterity save, and 5d8 a second to a dummy in it.",
        ),
        S::Polymorph => (
            "polymorph-icon",
            "Turns the dummy ahead into a harmless little beast for 8 seconds unless it makes a Wisdom save.",
        ),
        S::MassCureWounds => (
            "mass-cure-wounds-icon",
            "A wave of green light heals every dummy within 30 feet of the one ahead for 5d8 + 5.",
        ),
        S::Sunbeam => (
            "sunbeam-icon",
            "A 60-foot beam of sunlight: 6d8 radiant and blinded for 2 seconds, or half and unblinded on a Constitution save.",
        ),
        S::WallOfThorns => (
            "wall-of-thorns-icon",
            "Raises a 60-foot wall of brambles across the dummy ahead: 7d8 piercing as it grows, half on a Dexterity save, and 7d8 to a dummy pushed through it.",
        ),
        S::FireStorm => (
            "fire-storm-icon",
            "Sheets of flame fill a 20-foot radius at the dummy ahead: 7d10 fire, half on a Dexterity save.",
        ),
        S::ReverseGravity => (
            "reverse-gravity-icon",
            "Gravity flips in a 50-foot cylinder around you, so you and every dummy in it fall upward.",
        ),
        S::Sunburst => (
            "sunburst-icon",
            "Sunlight bursts 60 feet around the dummy ahead: 12d6 radiant and blinded for 3 seconds, or half and unblinded on a Constitution save.",
        ),
        S::StormOfVengeance => (
            "storm-of-vengeance-icon",
            "A churning storm over the dummy ahead: thunder, then acid rain, lightning, and hail, one every 6 seconds.",
        ),
        S::Shapechange => (
            "shapechange-icon",
            "Become a dragon three times your height in a whirl of leaves and light: fly with Jump and X, and breathe fire, bite, sweep, buffet, and roar while your spells still cast.",
        ),
        S::SpeakWithAnimals => (
            "speak-with-animals-icon",
            "Hear what the meadow's birds have to say about the dummies.",
        ),
        S::MeteorSwarm => (
            "meteor-swarm-icon",
            "Aim at the ground or a wall, even the tower's side, and click: six blazing meteors blast craters out of whatever they hit, and an undercut tower topples.",
        ),
        S::Thunderbolt => (
            "thunderbolt-icon",
            "Aim at the ground or a wall and click: a huge bolt of lightning blasts a chunk out of what it strikes and shocks the dummies there.",
        ),
        S::FireBolt => (
            "fire-bolt-icon",
            "Hurls a bolt at the dummy ahead: a spell attack for 4d10 fire, doubled on a natural 20.",
        ),
        S::BurningHands => (
            "burning-hands-icon",
            "A 15-foot cone of flame from your hands: 3d6 fire, half on a Dexterity save.",
        ),
        S::Blur => (
            "blur-icon",
            "Your body blurs so attacks miss it more often; the dummies don't attack, so a labeled burst.",
        ),
        S::Fireball => (
            "fireball-icon",
            "Throws a bead that bursts on the dummy ahead for 8d6 fire in a 20-foot radius, half on a Dexterity save.",
        ),
        S::Blight => (
            "blight-icon",
            "Withers the dummy ahead: 8d8 necrotic, half on a Constitution save.",
        ),
        S::WallOfStone => (
            "wall-of-stone-icon",
            "Raises granite panels at the dummy ahead that shove it out past the wall and block the way.",
        ),
        S::RayOfFrost => (
            "ray-of-frost-icon",
            "A frigid ray: a spell attack for 4d8 cold that slows the dummy for 6 seconds.",
        ),
        S::FogCloud => (
            "fog-cloud-icon",
            "A 20-foot radius of fog at the dummy ahead blinds every dummy inside while it lasts.",
        ),
        S::HoldPerson => (
            "hold-person-icon",
            "Holds the dummy ahead paralyzed for 6 seconds unless it makes a Wisdom save.",
        ),
        S::SleetStorm => (
            "sleet-storm-icon",
            "Sleet lashes a 40-foot radius at the dummy ahead, and every 3 seconds a dummy failing a Dexterity save slips and falls.",
        ),
        S::ConeOfCold => (
            "cone-of-cold-icon",
            "A 60-foot cone of cold from your hands: 8d8 cold, half on a Constitution save.",
        ),
        S::ShockingGrasp => (
            "shocking-grasp-icon",
            "Lightning leaps from your hand to the dummy at hand: a spell attack for 4d8 lightning.",
        ),
        S::Sleep => (
            "sleep-icon",
            "Puts dummies within 5 feet of the one ahead to sleep for 6 seconds unless they make a Wisdom save; damage wakes them.",
        ),
        S::MistyStep => (
            "misty-step-icon",
            "Blink up to 30 feet forward, stopping just short of the dummy ahead.",
        ),
        S::LightningBolt => (
            "lightning-bolt-icon",
            "A 100-foot bolt of lightning from you: 8d6 lightning, half on a Dexterity save.",
        ),
        S::FreedomOfMovement => (
            "freedom-of-movement-icon",
            "Nothing can slow or hold you; the dummies don't try, so a labeled burst.",
        ),
        S::TreeStride => (
            "tree-stride-icon",
            "Step into the meadow's trees and out again up to 60 feet toward the dummy ahead.",
        ),
        S::AcidSplash => (
            "acid-splash-icon",
            "A bubble of acid bursts on the dummy ahead: 4d6 acid within 5 feet unless a dummy makes a Dexterity save.",
        ),
        S::RayOfSickness => (
            "ray-of-sickness-icon",
            "A sickly ray: a spell attack for 2d8 poison that poisons the dummy for 6 seconds.",
        ),
        S::Web => (
            "web-icon",
            "Fills a 20-foot cube at the dummy ahead with webs that root each one failing a Dexterity save for 12 seconds.",
        ),
        S::StinkingCloud => (
            "stinking-cloud-icon",
            "A 20-foot radius of reeking gas at the dummy ahead poisons each dummy inside that fails a Constitution save.",
        ),
        S::InsectPlague => (
            "insect-plague-icon",
            "A swarm of locusts fills a 20-foot radius at the dummy ahead: 4d10 piercing a second, half on a Constitution save.",
        ),
        S::BearBite => (
            "bear-bite-icon",
            "Bite the dummy at your jaws: a +5 attack for 1d8 + 3 piercing.",
        ),
        S::BearClaw => (
            "bear-claw-icon",
            "Swipe the dummy at your paws: a +5 attack for 1d6 + 3 slashing that knocks it down.",
        ),
        S::WolfBite => (
            "wolf-bite-icon",
            "Bite the dummy at your jaws: a +5 attack for 1d10 + 3 piercing that knocks it down for 1.5 seconds.",
        ),
        S::EagleTalons => (
            "eagle-talons-icon",
            "Rake the dummy below with your talons: a +5 attack for 2d6 + 3 slashing.",
        ),
        S::SpiderBite => (
            "spider-bite-icon",
            "Bite the dummy at your fangs: a +5 attack for 1d8 + 3 piercing and 2d6 poison.",
        ),
        S::SpiderWeb => (
            "spider-web-icon",
            "Spit a web at a dummy up to 60 feet away: a +5 attack that roots it for 6 seconds.",
        ),
        S::DragonBite => (
            "dragon-bite-icon",
            "Snap your jaws on the dummy ahead: a +14 attack for 2d10 + 8 piercing and 2d6 fire.",
        ),
        S::FireBreath => (
            "fire-breath-icon",
            "Breathe a 60-foot cone of fire: 18d6 fire, half on a Dexterity save, and a failed save leaves a dummy burning.",
        ),
        S::TailSweep => (
            "tail-sweep-icon",
            "Swing your tail around you: 2d8 + 8 bludgeoning within 25 feet, half on a Dexterity save, and a failure knocks a dummy down.",
        ),
        S::WingBuffet => (
            "wing-buffet-icon",
            "Beat your wings down: 2d6 + 8 bludgeoning within 20 feet, half on a Dexterity save, and a failure throws a dummy back.",
        ),
        S::Roar => (
            "dragon-roar-icon",
            "Roar: each dummy within 60 feet that fails a Wisdom save cowers, frightened, for 4 seconds.",
        ),
    }
}
