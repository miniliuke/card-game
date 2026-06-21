//! 行动 API：apply_action 单一入口 + resume 续接。

use crate::rules::card::CardLevel;
use crate::rules::color::{CardColor, GemColor, PlayerId};
use crate::rules::error::RuleError;
use crate::rules::events::GameEvent;
use crate::rules::noble::NobleId;
use crate::rules::player::TOKEN_LIMIT;
use crate::rules::scoring::{eligible_nobles, standings};
use crate::rules::state::GameState;
use crate::rules::token::TokenSet;
use crate::rules::validation::{
    can_afford, can_reserve, can_take_three_different, can_take_two_same, plan_payment,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayerAction {
    TakeThreeDifferentTokens(Vec<GemColor>),
    TakeTwoSameTokens(GemColor),
    ReserveVisibleCard { level: crate::rules::card::CardLevel, idx: usize },
    ReserveDeckCard(crate::rules::card::CardLevel),
    BuyVisibleCard { level: crate::rules::card::CardLevel, idx: usize },
    BuyReservedCard(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ActionOutcome {
    Complete,
    NeedDiscardTokens { excess: u8 },
    NeedChooseNoble { candidates: Vec<crate::rules::noble::NobleId> },
    NeedFinalDiscardThenChooseNoble { excess: u8, candidates: Vec<crate::rules::noble::NobleId> },
}

impl ActionOutcome {
    pub fn requires_choice(&self) -> bool {
        !matches!(self, ActionOutcome::Complete)
    }
}

#[derive(Clone, Debug)]
pub struct ActionResult {
    pub outcome: ActionOutcome,
    pub events: Vec<GameEvent>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Resume {
    DiscardTokens(crate::rules::token::TokenSet),
    ChooseNoble(crate::rules::noble::NobleId),
}

pub fn apply_action(
    state: &mut GameState,
    player: PlayerId,
    action: PlayerAction,
) -> Result<ActionResult, RuleError> {
    if state.is_over() {
        return Err(RuleError::GameOver);
    }
    if player != state.current_id() {
        return Err(RuleError::NotYourTurn);
    }
    validate(&action, state, player)?;
    let mut events = Vec::new();
    let outcome = execute(&action, state, player, &mut events)?;
    Ok(ActionResult { outcome, events })
}

/// 顶层动作合法性校验：检查是否轮到该玩家、是否可执行该动作，不改 GameState。
pub fn validate_action(
    state: &GameState,
    player: PlayerId,
    action: &PlayerAction,
) -> Result<(), RuleError> {
    if state.is_over() {
        return Err(RuleError::GameOver);
    }
    if player != state.current_id() {
        return Err(RuleError::NotYourTurn);
    }
    validate(action, state, player)
}

pub fn resume(
    state: &mut GameState,
    player: PlayerId,
    resume: Resume,
) -> Result<ActionResult, RuleError> {
    if state.is_over() {
        return Err(RuleError::GameOver);
    }
    if player != state.current_id() {
        return Err(RuleError::NotYourTurn);
    }
    let mut events = Vec::new();
    match resume {
        // 弃牌只发生在"拿筹码/保留得金"后；这些行动不触发贵族、不触发终局。
        // 故弃牌后只归还筹码并推进回合，无需 check_nobles/end_game。
        Resume::DiscardTokens(returned) => {
            let excess = state.player(player).token_total().saturating_sub(TOKEN_LIMIT);
            if returned.total() != excess {
                return Err(RuleError::InvalidResume);
            }
            // 逐色归还（含金），玩家须持有。
            for color in GemColor::NORMAL {
                let amt = returned.get(color);
                if amt > 0 {
                    if !state.player_mut(player).tokens.remove(color, amt) {
                        return Err(RuleError::InvalidResume);
                    }
                    state.bank.give(color, amt);
                }
            }
            let gold = returned.get(GemColor::Gold);
            if gold > 0 {
                if !state.player_mut(player).tokens.remove(GemColor::Gold, gold) {
                    return Err(RuleError::InvalidResume);
                }
                state.bank.give(GemColor::Gold, gold);
            }
            events.push(GameEvent::TokensReturned { player, tokens: returned });
            advance_turn(state);
            maybe_finalize(state, &mut events);
        }
        // 选贵族发生在买牌后；授予贵族后继续终局检测 + 推进。
        Resume::ChooseNoble(noble_id) => {
            let bonus = state.player(player).bonus(&state.card_store);
            let candidates = state.nobles.eligible(bonus);
            if !candidates.contains(&noble_id) {
                return Err(RuleError::NobleNotEligible);
            }
            grant_noble(state, player, noble_id, &mut events);
            check_end_and_advance(state, player, &mut events)?;
        }
    }
    Ok(ActionResult { outcome: ActionOutcome::Complete, events })
}

fn validate(action: &PlayerAction, state: &GameState, player: PlayerId) -> Result<(), RuleError> {
    match action {
        PlayerAction::TakeThreeDifferentTokens(colors) => {
            can_take_three_different(state.player(player).tokens, state.bank.tokens, colors)
        }
        PlayerAction::TakeTwoSameTokens(color) => can_take_two_same(state.bank.tokens, *color),
        PlayerAction::ReserveVisibleCard { level, idx } => {
            can_reserve(state.player(player).reserved_cards.len())?;
            if state.market.visible(*level).get(*idx).is_none() {
                return Err(RuleError::CardNotFound);
            }
            Ok(())
        }
        PlayerAction::ReserveDeckCard(level) => {
            can_reserve(state.player(player).reserved_cards.len())?;
            if state.decks.remaining(*level) == 0 {
                return Err(RuleError::DeckEmpty);
            }
            Ok(())
        }
        PlayerAction::BuyVisibleCard { level, idx } => {
            let card = state
                .market
                .visible(*level)
                .get(*idx)
                .ok_or(RuleError::CardNotFound)?;
            let bonus = state.player(player).bonus(&state.card_store);
            can_afford(state.player(player).tokens, card, bonus)
        }
        PlayerAction::BuyReservedCard(reserved_idx) => {
            let &card_id = state
                .player(player)
                .reserved_cards
                .get(*reserved_idx)
                .ok_or(RuleError::CardNotFound)?;
            let card = state.card_store.get(card_id).ok_or(RuleError::CardNotFound)?;
            let bonus = state.player(player).bonus(&state.card_store);
            can_afford(state.player(player).tokens, card, bonus)
        }
    }
}

fn execute(
    action: &PlayerAction,
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> Result<ActionOutcome, RuleError> {
    match action {
        PlayerAction::TakeThreeDifferentTokens(colors) => {
            let mut taken = TokenSet::default();
            for c in colors {
                state.bank.take(*c, 1);
                state.player_mut(player).tokens.add(*c, 1);
                taken.add(*c, 1);
            }
            events.push(GameEvent::TokensTaken { player, tokens: taken });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::TakeTwoSameTokens(color) => {
            state.bank.take(*color, 2);
            state.player_mut(player).tokens.add(*color, 2);
            let mut taken = TokenSet::default();
            taken.add(*color, 2);
            events.push(GameEvent::TokensTaken { player, tokens: taken });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::ReserveVisibleCard { level, idx } => {
            let card = state.market.take(*level, *idx).ok_or(RuleError::CardNotFound)?;
            state.player_mut(player).reserved_cards.push(card.id);
            let got_gold = reserve_gold(state, player);
            if let Some(new_id) = state.market.refill(*level, &mut state.decks) {
                events.push(GameEvent::MarketRefilled { level: *level, card: Some(new_id) });
            } else {
                events.push(GameEvent::MarketRefilled { level: *level, card: None });
            }
            events.push(GameEvent::CardReserved { player, card: card.id, from_deck: false, got_gold });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::ReserveDeckCard(level) => {
            let card = state.decks.pop(*level).ok_or(RuleError::DeckEmpty)?;
            state.player_mut(player).reserved_cards.push(card.id);
            let got_gold = reserve_gold(state, player);
            events.push(GameEvent::CardReserved { player, card: card.id, from_deck: true, got_gold });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::BuyVisibleCard { level, idx } => {
            let card = state.market.take(*level, *idx).ok_or(RuleError::CardNotFound)?;
            buy_card(state, player, card, events, true, *level)
        }
        PlayerAction::BuyReservedCard(reserved_idx) => {
            let card_id = *state
                .player(player)
                .reserved_cards
                .get(*reserved_idx)
                .ok_or(RuleError::CardNotFound)?;
            let card = *state.card_store.get(card_id).ok_or(RuleError::CardNotFound)?;
            state.player_mut(player).reserved_cards.remove(*reserved_idx);
            buy_card(state, player, card, events, false, card.level)
        }
    }
}

fn reserve_gold(state: &mut GameState, player: PlayerId) -> bool {
    if state.bank.take(GemColor::Gold, 1) {
        state.player_mut(player).tokens.add(GemColor::Gold, 1);
        true
    } else {
        false
    }
}

/// 拿筹码/保留后：若超 TOKEN_LIMIT 则挂起弃牌；否则推进回合。
/// 这些行动不触发贵族/终局，但终局轮的最后一手可能正是"拿筹码"行动——
/// 故推进后仍需检查是否到达终局轮结算点。
fn discard_or_finish_tokens(
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> ActionOutcome {
    let total = state.player(player).token_total();
    if total > TOKEN_LIMIT {
        return ActionOutcome::NeedDiscardTokens { excess: total - TOKEN_LIMIT };
    }
    advance_turn(state);
    maybe_finalize(state, events);
    ActionOutcome::Complete
}

fn buy_card(
    state: &mut GameState,
    player: PlayerId,
    card: crate::rules::card::DevelopmentCard,
    events: &mut Vec<GameEvent>,
    from_market: bool,
    level: CardLevel,
) -> Result<ActionOutcome, RuleError> {
    let bonus = state.player(player).bonus(&state.card_store);
    let (paid, _gold) = plan_payment(state.player(player).tokens, &card, bonus);
    for color in CardColor::ALL {
        let amt = paid.get(color.to_gem());
        if amt > 0 {
            state.player_mut(player).tokens.remove(color.to_gem(), amt);
            state.bank.give(color.to_gem(), amt);
        }
    }
    let gold_used = paid.get(GemColor::Gold);
    if gold_used > 0 {
        state.player_mut(player).tokens.remove(GemColor::Gold, gold_used);
        state.bank.give(GemColor::Gold, gold_used);
    }
    state.player_mut(player).purchased_cards.push(card.id);
    events.push(GameEvent::CardPurchased { player, card: card.id, paid });
    if from_market {
        if let Some(new_id) = state.market.refill(level, &mut state.decks) {
            events.push(GameEvent::MarketRefilled { level, card: Some(new_id) });
        } else {
            events.push(GameEvent::MarketRefilled { level, card: None });
        }
    }
    // 买牌只减筹码，不会触发弃牌；只需贵族选择 + 终局 + 推进。
    finish_after_buy(state, player, events)
}

/// 买牌后：检查贵族（可能挂起 NeedChooseNoble）、终局检测、推进回合。
fn finish_after_buy(
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> Result<ActionOutcome, RuleError> {
    let candidates = eligible_nobles(state.player(player), &state.nobles, &state.card_store);
    match candidates.len() {
        0 => {
            check_end_and_advance(state, player, events)?;
            Ok(ActionOutcome::Complete)
        }
        1 => {
            grant_noble(state, player, candidates[0], events);
            check_end_and_advance(state, player, events)?;
            Ok(ActionOutcome::Complete)
        }
        _ => Ok(ActionOutcome::NeedChooseNoble { candidates }),
    }
}

fn grant_noble(state: &mut GameState, player: PlayerId, noble_id: NobleId, events: &mut Vec<GameEvent>) {
    if state.nobles.take(noble_id).is_some() {
        state.player_mut(player).nobles.push(noble_id);
        events.push(GameEvent::NobleVisited { player, noble: noble_id });
    }
}

/// 终局检测 + 回合推进 + 结算。
/// 终局轮：某玩家达 15 分后 end_triggered=true 并记录 final_player；
/// 此后继续行动，直到 advance_turn 使 current_player 再次回到 final_player
/// （即触发者之后的玩家都已完成一轮），结算胜负。
fn check_end_and_advance(
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> Result<(), RuleError> {
    let score = state.player(player).score(&state.card_store, &state.noble_store);
    if !state.end_triggered && score >= GameState::win_score() {
        state.end_triggered = true;
        state.final_player = Some(player);
        events.push(GameEvent::EndGameTriggered { player });
    }
    advance_turn(state);
    maybe_finalize(state, events);
    Ok(())
}

fn advance_turn(state: &mut GameState) {
    let n = state.players.len();
    state.current_player = (state.current_player + 1) % n;
}

/// 终局轮结算检查：end_triggered 且 current 回到 final_player 且未结算时，结算。
fn maybe_finalize(state: &mut GameState, events: &mut Vec<GameEvent>) {
    if state.end_triggered
        && state.current_player == state.final_player.unwrap_or(0)
        && state.winner.is_none()
    {
        finalize_game(state, events);
    }
}

fn finalize_game(state: &mut GameState, events: &mut Vec<GameEvent>) {
    let s = standings(&state.players, &state.card_store, &state.noble_store);
    let winner = s.first().map(|(id, _)| *id);
    state.winner = winner;
    events.push(GameEvent::GameOver { winner: winner.unwrap_or(0), standings: s });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{CardLevel, GemCost};
    use crate::rules::color::GemColor;
    use crate::rules::noble::Noble;
    use crate::rules::token::TokenSet;

    fn game2() -> GameState {
        GameState::new_seeded(2, 99).unwrap()
    }

    #[test]
    fn take_three_different_moves_tokens_from_bank() {
        let mut g = game2();
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        assert!(matches!(r.outcome, ActionOutcome::Complete));
        assert_eq!(g.player(0).token_count(GemColor::White), 1);
        assert_eq!(g.bank.tokens.white, 3);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::TokensTaken { player: 0, .. })));
    }

    #[test]
    fn take_three_different_rejects_duplicate() {
        let mut g = game2();
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::White, GemColor::Blue]));
        assert_eq!(r.unwrap_err(), RuleError::InvalidTokenSelection);
    }

    #[test]
    fn take_two_same_needs_four_in_bank() {
        let mut g = game2();
        let r = apply_action(&mut g, 0, PlayerAction::TakeTwoSameTokens(GemColor::White));
        // 2 人局每色 4，应成功。
        assert!(r.is_ok());
        assert_eq!(g.player(0).token_count(GemColor::White), 2);
    }

    #[test]
    fn take_two_same_fails_below_four() {
        // 先取 1 个白使银行降到 3。
        let mut g = game2();
        g.bank.tokens.white = 3;
        let r = apply_action(&mut g, 0, PlayerAction::TakeTwoSameTokens(GemColor::White));
        assert_eq!(r.unwrap_err(), RuleError::BankInsufficient);
    }

    #[test]
    fn reserve_visible_takes_gold_and_refills_immediately() {
        let mut g = game2();
        let gold_before = g.bank.tokens.gold;
        let r = apply_action(&mut g, 0, PlayerAction::ReserveVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        assert_eq!(g.player(0).reserved_cards.len(), 1);
        assert_eq!(g.player(0).token_count(GemColor::Gold), 1);
        assert_eq!(g.bank.tokens.gold, gold_before - 1);
        // 立即补牌：可见仍 4 张。
        assert_eq!(g.market.visible(CardLevel::Level1).len(), 4);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::CardReserved { from_deck: false, got_gold: true, .. })));
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::MarketRefilled { .. })));
    }

    #[test]
    fn reserve_deck_blinds_top() {
        let mut g = game2();
        let before = g.decks.remaining(CardLevel::Level1);
        let r = apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        assert_eq!(g.player(0).reserved_cards.len(), 1);
        assert_eq!(g.decks.remaining(CardLevel::Level1), before - 1);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::CardReserved { from_deck: true, .. })));
    }

    #[test]
    fn reserve_limit_is_three() {
        let mut g = game2();
        // 每次保留都会推进回合，故玩家 0/1 交替。玩家 0 在第 1、3、5 手保留达 3 张。
        apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        apply_action(&mut g, 1, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        apply_action(&mut g, 1, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap()
            ;
        apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        assert_eq!(g.player(0).reserved_cards.len(), 3);
        // 玩家 1 行动后轮到 0，此时 0 已满 3，再保留应被拒。
        apply_action(&mut g, 1, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        let r = apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1));
        assert_eq!(r.unwrap_err(), RuleError::TooManyReserved);
    }

    #[test]
    fn buy_visible_pays_discount_and_gold_to_bank() {
        // 构造：玩家持白2蓝2金1，买一张白3蓝2的卡（bonus 白色=0）。
        let mut g = game2();
        // 放一张已知卡到市场第 0 位。
        let card = crate::rules::card::DevelopmentCard {
            id: 999,
            level: CardLevel::Level1,
            color: crate::rules::color::CardColor::White,
            prestige: 1,
            cost: GemCost { white: 0, blue: 2, green: 0, red: 3, black: 0 },
        };
        g.market.level1_visible[0] = card;
        g.card_store = crate::rules::card::CardStore::from_cards(&[card]);
        // 给玩家白2 蓝2 红2 金2 以支付 红3（白0 蓝2 红2 不足红1，金补1）
        g.players[0].tokens = TokenSet { white: 2, blue: 2, red: 2, gold: 2, ..Default::default() };
        let bank_before = g.bank.tokens;
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        assert!(matches!(r.outcome, ActionOutcome::Complete));
        assert!(g.player(0).purchased_cards.contains(&999));
        assert_eq!(g.player(0).token_count(GemColor::Red), 0); // 红2 全付
        assert_eq!(g.player(0).token_count(GemColor::Blue), 0); // 蓝2 全付
        assert_eq!(g.player(0).token_count(GemColor::Gold), 1); // 金2 付1 剩1
        // 支付的筹码回到银行。
        assert_eq!(g.bank.tokens.red, bank_before.red + 2);
        assert_eq!(g.bank.tokens.blue, bank_before.blue + 2);
        assert_eq!(g.bank.tokens.gold, bank_before.gold + 1);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::CardPurchased { player: 0, card: 999, .. })));
    }

    #[test]
    fn buy_cannot_afford() {
        let mut g = game2();
        let card = crate::rules::card::DevelopmentCard {
            id: 999,
            level: CardLevel::Level1,
            color: crate::rules::color::CardColor::White,
            prestige: 0,
            cost: GemCost { white: 0, blue: 5, green: 0, red: 0, black: 0 },
        };
        g.market.level1_visible[0] = card;
        g.card_store = crate::rules::card::CardStore::from_cards(&[card]);
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 });
        assert_eq!(r.unwrap_err(), RuleError::CannotAfford);
    }

    #[test]
    fn token_limit_triggers_discard() {
        // 玩家已持 9 筹码，拿 3 不同 -> 12，超 10，excess=2。
        let mut g = game2();
        g.players[0].tokens = TokenSet { white: 3, blue: 3, green: 3, ..Default::default() }; // 9 个
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        assert!(matches!(r.outcome, ActionOutcome::NeedDiscardTokens { excess: 2 }));
        // 回合未推进（仍玩家 0）。
        assert_eq!(g.current_player, 0);
    }

    #[test]
    fn end_game_triggers_at_fifteen_and_finishes_round() {
        // 玩家 0 直接给 14 分，买一张 1 分卡 -> 15 触发。
        let mut g = game2();
        // 给玩家 0 一张已购的 14 分卡（构造 store 支持）。
        let big = crate::rules::card::DevelopmentCard { id: 1000, level: CardLevel::Level3, color: crate::rules::color::CardColor::White, prestige: 14, cost: GemCost::default() };
        let target = crate::rules::card::DevelopmentCard { id: 1001, level: CardLevel::Level1, color: crate::rules::color::CardColor::White, prestige: 1, cost: GemCost::default() };
        g.card_store = crate::rules::card::CardStore::from_cards(&[big, target]);
        g.players[0].purchased_cards.push(1000);
        g.market.level1_visible[0] = target;
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        assert!(g.end_triggered);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::EndGameTriggered { player: 0 })));
        // 2 人局：玩家 0 触发后，玩家 1 还需行动一次才结算。当前应轮到 1。
        assert_eq!(g.current_player, 1);
        assert!(g.winner.is_none());
        // 玩家 1 行动后结算。
        apply_action(&mut g, 1, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        assert!(g.is_over());
        assert_eq!(g.winner, Some(0));
    }

    #[test]
    fn resume_discard_returns_tokens_and_advances() {
        let mut g = game2();
        g.players[0].tokens = TokenSet { white: 3, blue: 3, green: 3, ..Default::default() }; // 9
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        let excess = match r.outcome { ActionOutcome::NeedDiscardTokens { excess } => excess, _ => panic!() };
        assert_eq!(excess, 2);
        let returned = TokenSet { white: 2, ..Default::default() };
        let r2 = resume(&mut g, 0, Resume::DiscardTokens(returned)).unwrap();
        assert!(matches!(r2.outcome, ActionOutcome::Complete));
        assert_eq!(g.player(0).token_total(), 10);
        assert_eq!(g.current_player, 1);
    }

    #[test]
    fn resume_choose_noble_grants_noble() {
        let mut g = game2();
        // 构造玩家满足两个贵族。
        let big = crate::rules::card::DevelopmentCard { id: 2000, level: CardLevel::Level3, color: crate::rules::color::CardColor::White, prestige: 0, cost: GemCost::default() };
        g.card_store = crate::rules::card::CardStore::from_cards(&[big]);
        // 给玩家白色 4 蓝色 4 已购卡（满足 4W4B 与另一 3W3B3G? 此处仅造一个双候选场景）
        for _ in 0..4 {
            g.players[0].purchased_cards.push(2000); // 白色 +4
        }
        // 贵族：放两个 4W4B 与 3W3B3G 都需满足——简化：放两个相同要求 4W4B 的可用贵族。
        let n1 = crate::rules::noble::Noble { id: 50, prestige: 3, requirement: GemCost { white: 4, blue: 0, green: 0, red: 0, black: 0 } };
        let n2 = crate::rules::noble::Noble { id: 51, prestige: 3, requirement: GemCost { white: 4, blue: 0, green: 0, red: 0, black: 0 } };
        g.nobles = crate::rules::noble::NobleBoard { available: vec![n1, n2], taken: vec![] };
        g.noble_store = crate::rules::noble::NobleStore::from_nobles(&[n1, n2]);
        // 买一张 0 分卡触发贵族检查。
        let target = crate::rules::card::DevelopmentCard { id: 2001, level: CardLevel::Level1, color: crate::rules::color::CardColor::White, prestige: 0, cost: GemCost::default() };
        g.market.level1_visible[0] = target;
        g.card_store = crate::rules::card::CardStore::from_cards(&[big, target]);
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        let cands = match r.outcome { ActionOutcome::NeedChooseNoble { candidates } => candidates, _ => panic!("expected noble choice") };
        assert_eq!(cands.len(), 2);
        let r2 = resume(&mut g, 0, Resume::ChooseNoble(50)).unwrap();
        assert!(matches!(r2.outcome, ActionOutcome::Complete));
        assert!(g.player(0).nobles.contains(&50));
    }
}
