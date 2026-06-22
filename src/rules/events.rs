//! 规则层产生的事件，供 UI 据此播放动画。

use crate::rules::card::{CardId, CardLevel};
use crate::rules::color::PlayerId;
use crate::rules::noble::NobleId;
use crate::rules::player::ReserveOrigin;
use crate::rules::token::TokenSet;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GameEvent {
    TokensTaken {
        player: PlayerId,
        tokens: TokenSet,
    },
    TokensReturned {
        player: PlayerId,
        tokens: TokenSet,
    },
    CardReserved {
        player: PlayerId,
        card: CardId,
        origin: ReserveOrigin,
        got_gold: bool,
    },
    CardPurchased {
        player: PlayerId,
        card: CardId,
        paid: TokenSet,
    },
    MarketRefilled {
        level: CardLevel,
        card: Option<CardId>,
    },
    NobleVisited {
        player: PlayerId,
        noble: NobleId,
    },
    EndGameTriggered {
        player: PlayerId,
    },
    GameOver {
        winner: PlayerId,
        standings: Vec<(PlayerId, u16)>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_construct_with_all_fields() {
        let e = GameEvent::CardPurchased {
            player: 0,
            card: 7,
            paid: TokenSet::default(),
        };
        assert!(matches!(
            e,
            GameEvent::CardPurchased {
                player: 0,
                card: 7,
                ..
            }
        ));
        let g = GameEvent::GameOver {
            winner: 1,
            standings: vec![(1, 15), (0, 12)],
        };
        assert!(matches!(g, GameEvent::GameOver { winner: 1, .. }));
    }
}
