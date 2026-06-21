//! 贵族牌与公共贵族区。

use std::collections::HashMap;

use crate::rules::card::{CardBonus, GemCost};

pub type NobleId = u8;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Noble {
    pub id: NobleId,
    pub prestige: u8,
    pub requirement: GemCost,
}

#[derive(Clone, Default, Debug)]
pub struct NobleStore {
    map: HashMap<NobleId, Noble>,
}

impl NobleStore {
    pub fn from_nobles(nobles: &[Noble]) -> Self {
        let map = nobles.iter().copied().map(|n| (n.id, n)).collect();
        Self { map }
    }

    pub fn get(&self, id: NobleId) -> Option<&Noble> {
        self.map.get(&id)
    }
}

/// 公共贵族区：可见可被拜访的贵族 + 已被带走的（便于 UI 展示）。
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct NobleBoard {
    pub available: Vec<Noble>,
    pub taken: Vec<NobleId>,
}

impl NobleBoard {
    pub fn take(&mut self, id: NobleId) -> Option<Noble> {
        let pos = self.available.iter().position(|n| n.id == id)?;
        let noble = self.available.remove(pos);
        self.taken.push(id);
        Some(noble)
    }

    /// 返回玩家 bonus 已满足的 available 贵族 id。
    pub fn eligible(&self, bonus: CardBonus) -> Vec<NobleId> {
        self.available
            .iter()
            .filter(|n| bonus.satisfies(n.requirement))
            .map(|n| n.id)
            .collect()
    }
}

/// 标准贵族牌池（10 张，各 3 分）。requirement 为 [W,B,G,R,K]。
/// 数值由作者凭记忆录入，可能存在偏差；统计特征由测试锁定（10 张、皆 3 分）。
pub fn standard_nobles() -> Vec<Noble> {
    let mk = |id: NobleId, req: [u8; 5]| Noble {
        id,
        prestige: 3,
        requirement: GemCost { white: req[0], blue: req[1], green: req[2], red: req[3], black: req[4] },
    };
    vec![
        mk(0, [4, 4, 0, 0, 0]),
        mk(1, [4, 0, 4, 0, 0]),
        mk(2, [0, 4, 0, 4, 0]),
        mk(3, [0, 0, 4, 0, 4]),
        mk(4, [4, 0, 0, 0, 4]),
        mk(5, [3, 3, 3, 0, 0]),
        mk(6, [3, 0, 3, 3, 0]),
        mk(7, [0, 3, 0, 3, 3]),
        mk(8, [3, 0, 3, 0, 3]),
        mk(9, [0, 3, 3, 3, 0]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_nobles_are_ten_three_pointers() {
        let nobles = standard_nobles();
        assert_eq!(nobles.len(), 10);
        assert!(nobles.iter().all(|n| n.prestige == 3));
        let mut ids: Vec<_> = nobles.iter().map(|n| n.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn board_take_moves_to_taken() {
        let mut board = NobleBoard { available: standard_nobles(), taken: vec![] };
        let taken = board.take(0).unwrap();
        assert_eq!(taken.id, 0);
        assert!(board.available.iter().all(|n| n.id != 0));
        assert_eq!(board.taken, vec![0]);
    }

    #[test]
    fn eligible_filters_by_bonus() {
        let board = NobleBoard { available: standard_nobles(), taken: vec![] };
        let bonus = CardBonus { white: 4, blue: 4, ..Default::default() };
        let elig = board.eligible(bonus);
        assert!(elig.contains(&0)); // 4W 4B
        assert!(!elig.contains(&2));
    }
}
