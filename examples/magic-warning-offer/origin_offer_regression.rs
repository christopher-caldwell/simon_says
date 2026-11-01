use super::*;
use crate::card::CardInstance;
use forge_carddb::CardDatabase;
use forge_foundation::{CardTypeLine, ColorSet, ManaCost, ZoneType};

fn setup(with_artifact: bool) -> (GameState, GameLoop, PlayerId, CardId) {
    let archive = std::env::var("SIMON_MAGIC_ARCHIVE").expect("explicit archived card set");
    let cards = CardDatabase::load_from_archive(&std::fs::read(archive).unwrap()).unwrap();
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let origin = game.create_card(CardInstance::from_rules(
        cards.cards.get_by_card_name("Origin of Metalbending").expect("archived card"), player,
    ));
    game.move_card(origin, ZoneType::Hand, player);
    for _ in 0..2 {
        let forest = game.create_card(CardInstance::new(
            CardId(0), "Forest".into(), player, CardTypeLine::parse("Basic Land - Forest"),
            ManaCost::no_cost(), ColorSet::COLORLESS, None, None, vec![], vec![],
        ));
        game.move_card(forest, ZoneType::Battlefield, player);
    }
    if with_artifact {
        let target = game.create_card(CardInstance::new(
            CardId(0), "Test Artifact".into(), opponent, CardTypeLine::parse("Artifact"),
            ManaCost::no_cost(), ColorSet::COLORLESS, None, None, vec![], vec![],
        ));
        game.move_card(target, ZoneType::Battlefield, opponent);
    }
    (game, GameLoop::new(2), player, origin)
}

#[test]
fn archived_origin_not_offered_without_any_legal_mode() {
    let (game, runner, player, origin) = setup(false);
    let spell = game.card(origin).abilities.iter().find(|s| s.contains("SP$ Charm")).expect("archived modal spell");
    assert!(!crate::ability::effects::charm_effect::can_make_choices_precast(&game, player, origin, spell));
    assert!(!runner.get_playable_cards(&game, player, true).iter().any(|o| o.card_id == origin),
        "Origin must not be offered when no declared mode has a legal target");
}

#[test]
fn archived_origin_offered_when_artifact_target_is_available() {
    let (game, runner, player, origin) = setup(true);
    let spell = game.card(origin).abilities.iter().find(|s| s.contains("SP$ Charm")).expect("archived modal spell");
    assert!(crate::ability::effects::charm_effect::can_make_choices_precast(&game, player, origin, spell));
    assert!(runner.get_playable_cards(&game, player, true).iter().any(|o| o.card_id == origin),
        "Origin must remain available when a legal mode and target exist");
}

#[test]
fn archived_boros_charm_offered_when_non_target_mode_is_available() {
    let archive = std::env::var("SIMON_MAGIC_ARCHIVE").expect("explicit archived card set");
    let cards = CardDatabase::load_from_archive(&std::fs::read(archive).unwrap()).unwrap();
    let player = PlayerId(0);
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let charm = game.create_card(CardInstance::from_rules(
        cards.cards.get_by_card_name("Boros Charm").expect("archived card"), player,
    ));
    game.move_card(charm, ZoneType::Hand, player);
    for (name, kind) in [("Mountain", "Basic Land - Mountain"), ("Plains", "Basic Land - Plains")] {
        let land = game.create_card(CardInstance::new(
            CardId(0), name.into(), player, CardTypeLine::parse(kind), ManaCost::no_cost(),
            ColorSet::COLORLESS, None, None, vec![], vec![],
        ));
        game.move_card(land, ZoneType::Battlefield, player);
    }
    let spell = game.card(charm).abilities.iter().find(|s| s.contains("SP$ Charm")).expect("archived modal spell");
    assert!(crate::ability::effects::charm_effect::can_make_choices_precast(&game, player, charm, spell));
    assert!(GameLoop::new(2).get_playable_cards(&game, player, true).iter().any(|o| o.card_id == charm),
        "Boros Charm must remain available when at least one valid mode exists");
}
