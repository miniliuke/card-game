//! 计分、胜负比较、贵族资格。

use std::cmp::Ordering;

use crate::rules::card::CardStore;
use crate::rules::color::PlayerId;
use crate::rules::noble::{NobleBoard, NobleId, NobleStore};
use crate::rules::player::PlayerState;

/// 玩家总分 = 卡分 + 贵族分。
pub fn calculate_score(player: &PlayerState, cards: &CardStore, nobles: &NobleStore) -> u16 {
    player.score(cards, nobles)
}

/// 胜负排序：分数降序；分数相同则已购发展卡数升序（买牌少者胜）。
/// 返回 `Ordering` 用于 `sort_by`，使胜者排在最前。
pub fn compare_players(
    a: &PlayerState,
    b: &PlayerState,
    cards: &CardStore,
    nobles: &NobleStore,
) -> Ordering {
    let sa = calculate_score(a, cards, nobles);
    let sb = calculate_score(b, cards, nobles);
    sa.cmp(&sb)
        .reverse() // 降序：高分在前
        .then_with(|| {
            // 分数相同：买牌少者在前（升序）
            a.purchased_cards.len().cmp(&b.purchased_cards.len())
        })
}

/// 玩家 bonus 已满足的 available 贵族 id 列表。
pub fn eligible_nobles(
    player: &PlayerState,
    board: &NobleBoard,
    cards: &CardStore,
) -> Vec<NobleId> {
    let bonus = player.bonus(cards);
    board.eligible(bonus)
}

/// 返回按胜者优先排序的 (player_id, score) 列表。
pub fn standings(
    players: &[PlayerState],
    cards: &CardStore,
    nobles: &NobleStore,
) -> Vec<(PlayerId, u16)> {
    let mut indexed: Vec<&PlayerState> = players.iter().collect();
    indexed.sort_by(|a, b| compare_players(a, b, cards, nobles));
    indexed
        .iter()
        .map(|p| (p.id, calculate_score(p, cards, nobles)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{DevelopmentCard, GemCost};
    use crate::rules::color::CardColor;
    use crate::rules::noble::Noble;
    use crate::rules::player::PlayerState;

    fn stores() -> (CardStore, NobleStore) {
        let c = DevelopmentCard { id: 1, level: crate::rules::card::CardLevel::Level1, color: CardColor::White, prestige: 2, cost: GemCost::default() };
        let zero = DevelopmentCard { id: 2, level: crate::rules::card::CardLevel::Level1, color: CardColor::Blue, prestige: 0, cost: GemCost::default() };
        let n = Noble { id: 0, prestige: 3, requirement: GemCost::default() };
        (CardStore::from_cards(&[c, zero]), NobleStore::from_nobles(&[n]))
    }

    #[test]
    fn higher_score_ranks_first() {
        let (cs, ns) = stores();
        let mut a = PlayerState::new(0);
        a.purchased_cards.push(1); // 2 分
        let b = PlayerState::new(1);
        assert_eq!(compare_players(&a, &b, &cs, &ns), Ordering::Less); // a 在前
    }

    #[test]
    fn tie_broken_by_fewer_purchased() {
        let (cs, ns) = stores();
        let mut a = PlayerState::new(0);
        a.purchased_cards.push(1); // 2 分, 1 牌
        let mut b = PlayerState::new(1);
        b.purchased_cards.push(1); // 2 分
        b.purchased_cards.push(2); // +0 分 -> 同 2 分, 2 牌
        assert_eq!(compare_players(&a, &b, &cs, &ns), Ordering::Less); // a 买牌少在前
    }

    #[test]
    fn standings_winner_first() {
        let (cs, ns) = stores();
        let mut a = PlayerState::new(0);
        a.purchased_cards.push(1);
        let b = PlayerState::new(1);
        let s = standings(&[b, a], &cs, &ns);
        assert_eq!(s[0], (0, 2));
    }
}
