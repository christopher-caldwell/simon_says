//! Run against the standalone native harness and an explicit copied archive.
use forge_carddb::CardDatabase;
use forge_foundation::ZoneType;
use manabrew_engine::{card::CardInstance, game::GameState, ids::PlayerId, svar::resolve_count_svar};

#[test]
fn archived_rough_buddy_expression_counts_real_creature_departure() {
    let archive = std::env::var("SIMON_MAGIC_ARCHIVE").expect("explicit copied archive");
    let cards = CardDatabase::load_from_archive(&std::fs::read(archive).unwrap()).unwrap();
    let player = PlayerId(0);
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let spell = game.create_card(CardInstance::from_rules(
        cards.cards.get_by_card_name("That's Rough Buddy").expect("archived spell"), player));
    let creature = game.create_card(CardInstance::from_rules(
        cards.cards.get_by_card_name("Spore Frog").expect("archived creature"), player));
    let expression = game.card(spell).svars.get("Y").expect("archived Y SVar").clone();
    assert_eq!(expression, "Count$LeftBattlefieldThisTurn Creature.YouCtrl");
    game.move_card(creature, ZoneType::Battlefield, player);
    game.move_card(creature, ZoneType::Exile, player);
    assert_eq!(resolve_count_svar(&expression, &game, spell, player), 1);
    game.new_turn_for_player(player);
    assert_eq!(resolve_count_svar(&expression, &game, spell, player), 0);
}
