pub const LEVEL_COUNT: usize = 3;
pub const SLOTS_PER_LEVEL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemColor {
    White,
    Blue,
    Green,
    Red,
    Black,
}

impl GemColor {
    pub const ALL: [Self; 5] = [Self::White, Self::Blue, Self::Green, Self::Red, Self::Black];

    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: u32,
    pub name: String,
    pub level: u8,
    pub bonus: GemColor,
    pub points: u8,
    pub costs: [u8; 5],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealtCard {
    pub level: usize,
    pub slot: usize,
    pub card: Card,
}

#[derive(Debug, Clone, Default)]
pub struct PlayerState {
    cards: [u8; 5],
    tokens: [u8; 5],
    score: u16,
}

impl PlayerState {
    pub fn card_count(&self, color: GemColor) -> u8 {
        self.cards[color.index()]
    }

    pub fn score(&self) -> u16 {
        self.score
    }

    pub fn token_count(&self, color: GemColor) -> u8 {
        self.tokens[color.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    InvalidSlot,
    EmptySlot,
    TokenUnavailable,
}

pub struct GameSession {
    players: Vec<PlayerState>,
    market: Vec<Vec<Option<Card>>>,
    decks: Vec<Vec<Card>>,
    token_supply: [u8; 5],
    active_player: usize,
    round: u32,
}

impl GameSession {
    pub fn new(player_count: usize, seed: u64) -> Self {
        assert!(player_count > 0, "a game needs at least one player");
        let mut decks: Vec<Vec<Card>> = (0..LEVEL_COUNT)
            .map(|level| shuffled_level_deck(level, seed.wrapping_add(level as u64)))
            .collect();
        let mut market = vec![vec![None; SLOTS_PER_LEVEL]; LEVEL_COUNT];
        for level in 0..LEVEL_COUNT {
            for slot in 0..SLOTS_PER_LEVEL {
                market[level][slot] = decks[level].pop();
            }
        }

        Self {
            players: vec![PlayerState::default(); player_count],
            market,
            decks,
            token_supply: [(player_count + 2) as u8; 5],
            active_player: 0,
            round: 1,
        }
    }

    pub fn visible_card(&self, level: usize, slot: usize) -> Option<&Card> {
        self.market.get(level)?.get(slot)?.as_ref()
    }

    pub fn player(&self, player: usize) -> &PlayerState {
        &self.players[player]
    }

    pub fn take_card(&mut self, level: usize, slot: usize) -> Result<Card, GameError> {
        let card = self
            .market
            .get_mut(level)
            .and_then(|row| row.get_mut(slot))
            .ok_or(GameError::InvalidSlot)?
            .take()
            .ok_or(GameError::EmptySlot)?;
        let player = &mut self.players[self.active_player];
        player.cards[card.bonus.index()] += 1;
        player.score += u16::from(card.points);
        Ok(card)
    }

    pub fn token_supply(&self, color: GemColor) -> u8 {
        self.token_supply[color.index()]
    }

    pub fn take_token(&mut self, color: GemColor) -> Result<(), GameError> {
        let supply = &mut self.token_supply[color.index()];
        if *supply == 0 {
            return Err(GameError::TokenUnavailable);
        }
        *supply -= 1;
        self.players[self.active_player].tokens[color.index()] += 1;
        Ok(())
    }

    pub fn active_player(&self) -> usize {
        self.active_player
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    pub fn deck_remaining(&self, level: usize) -> usize {
        self.decks.get(level).map_or(0, Vec::len)
    }

    pub fn end_turn(&mut self) -> Vec<DealtCard> {
        let mut dealt = Vec::new();
        for level in 0..LEVEL_COUNT {
            for slot in 0..SLOTS_PER_LEVEL {
                if self.market[level][slot].is_none()
                    && let Some(card) = self.decks[level].pop()
                {
                    self.market[level][slot] = Some(card.clone());
                    dealt.push(DealtCard { level, slot, card });
                }
            }
        }

        self.active_player = (self.active_player + 1) % self.players.len();
        if self.active_player == 0 {
            self.round += 1;
        }
        dealt
    }
}

fn shuffled_level_deck(level: usize, seed: u64) -> Vec<Card> {
    let mut cards: Vec<Card> = (0..12).map(|index| make_card(level, index)).collect();
    let mut rng = seed.max(1);
    for index in (1..cards.len()).rev() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        cards.swap(index, rng as usize % (index + 1));
    }
    cards
}

fn make_card(level: usize, index: usize) -> Card {
    const NAMES: [&str; 12] = [
        "MOON VEIL",
        "EMBER OATH",
        "JADE ECHO",
        "IVORY STAR",
        "ONYX CROWN",
        "TIDAL RUNE",
        "VERDANT KEY",
        "SCARLET WARD",
        "PALE COMET",
        "NIGHT ORACLE",
        "AZURE LENS",
        "THORN SIGIL",
    ];
    let bonus = GemColor::ALL[(index + level * 2) % GemColor::ALL.len()];
    let mut costs = [0; 5];
    let base = (level + 1) as u8;
    for offset in 1..=3 {
        let color = (bonus.index() + offset + index) % 5;
        costs[color] = base + ((index + offset) % (level + 2)) as u8;
    }
    costs[bonus.index()] = 0;

    Card {
        id: ((level + 1) * 100 + index) as u32,
        name: NAMES[index].to_string(),
        level: (level + 1) as u8,
        bonus,
        points: match level {
            0 => (index % 2) as u8,
            1 => 1 + (index % 3) as u8,
            _ => 3 + (index % 3) as u8,
        },
        costs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taking_a_card_moves_its_reward_to_the_active_player() {
        let mut game = GameSession::new(2, 7);
        let card = game.visible_card(0, 0).cloned().expect("a visible card");
        let cards_before = game.player(0).card_count(card.bonus);
        let score_before = game.player(0).score();

        let taken = game.take_card(0, 0).expect("card can be taken");

        assert_eq!(taken.id, card.id);
        assert!(game.visible_card(0, 0).is_none());
        assert_eq!(game.player(0).card_count(card.bonus), cards_before + 1);
        assert_eq!(
            game.player(0).score(),
            score_before + u16::from(card.points)
        );
    }

    #[test]
    fn taking_a_token_transfers_one_from_supply_to_the_active_player() {
        let mut game = GameSession::new(2, 11);

        assert_eq!(game.token_supply(GemColor::Red), 4);

        game.take_token(GemColor::Red).expect("token is available");

        assert_eq!(game.token_supply(GemColor::Red), 3);
        assert_eq!(game.player(0).token_count(GemColor::Red), 1);
    }

    #[test]
    fn ending_a_turn_refills_empty_slots_and_switches_player() {
        let mut game = GameSession::new(2, 19);
        let old_card = game.take_card(1, 2).expect("card can be taken");

        let dealt = game.end_turn();

        let replacement = game.visible_card(1, 2).expect("slot is refilled");
        assert_ne!(replacement.id, old_card.id);
        assert_eq!(dealt.len(), 1);
        assert_eq!((dealt[0].level, dealt[0].slot), (1, 2));
        assert_eq!(game.active_player(), 1);
        assert_eq!(game.round(), 1);

        game.take_token(GemColor::Blue).expect("player two can act");
        assert_eq!(game.player(0).token_count(GemColor::Blue), 0);
        assert_eq!(game.player(1).token_count(GemColor::Blue), 1);

        game.end_turn();
        assert_eq!(game.active_player(), 0);
        assert_eq!(game.round(), 2);
    }

    #[test]
    fn an_exhausted_deck_leaves_the_market_slot_empty() {
        let mut game = GameSession::new(2, 23);

        for _ in 0..8 {
            game.take_card(2, 0).expect("visible card");
            game.end_turn();
        }
        game.take_card(2, 0).expect("last visible card");
        let dealt = game.end_turn();

        assert!(dealt.is_empty());
        assert_eq!(game.deck_remaining(2), 0);
        assert!(game.visible_card(2, 0).is_none());
    }
}
