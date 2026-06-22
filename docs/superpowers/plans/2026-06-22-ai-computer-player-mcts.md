# AI Computer Player with MO-ISMCTS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a fair hidden-information computer opponent for the existing two-player Splendor game, using cancellable single-threaded MO-ISMCTS with a one-second budget for each decision.

**Architecture:** Keep `GameState` authoritative on the Bevy main thread, derive a redacted `AiObservation`, and run a pure-Rust search on Bevy's async compute pool. The search re-determinizes hidden cards for the acting observer, submits only typed `AiDecision` values, and sends every result through the existing `apply_action`/`resume` rule APIs.

**Tech Stack:** Rust 2024, Bevy 0.18.1 task pools and ECS, `rand` 0.9, standard-library atomics and collections, built-in Rust test harness.

---

## Execution prerequisites

- Read the approved design first: `docs/superpowers/specs/2026-06-22-ai-computer-player-mcts-design.md`.
- The source workspace already has user-owned modifications in `Cargo.toml`, `src/battle.rs`, and `src/main.rs`, and this plan was written against that exact snapshot. Before creating an isolated worktree, ask the user to either commit those changes or name the commit/branch that already contains them. Do not commit, stash, overwrite or discard them without that direction.
- After the prerequisite snapshot is represented by a commit, create an isolated worktree from it with `superpowers:using-git-worktrees`.
- Run the baseline before editing:

```powershell
cargo test
git status --short
```

Expected: the existing test suite passes; the three pre-existing modified files remain visible in status.

## File responsibility map

| File | Responsibility |
|---|---|
| `src/rules/player.rs` | Preserve reservation identity and whether it was public or blind |
| `src/rules/actions.rs` | Apply reservation model and enumerate all legal main actions |
| `src/rules/events.rs` | Report reservation origin to presentation code |
| `src/rules/mod.rs` | Re-export rule types and standard static data needed by AI |
| `src/ai/mod.rs` | AI public API and shared error type |
| `src/ai/decision.rs` | Decision contexts, complete-turn simulation wrapper, discard enumeration |
| `src/ai/observation.rs` | Redacted per-player observations and stable information-set keys |
| `src/ai/determinization.rs` | Construct card-conserving possible worlds and actor-relative re-determinization |
| `src/ai/evaluation.rs` | Six-feature cutoff evaluation from the root player's perspective |
| `src/ai/rollout.rs` | Observation-safe heuristic rollout and guaranteed legal fallback |
| `src/ai/mcts.rs` | UCT tree, availability statistics, limits, cancellation, metrics and root choice |
| `src/battle/ai_runtime.rs` | Bevy background task lifecycle, request tokens and stale-result rejection |
| `src/battle.rs` | Unified rule-decision queue, AI runtime scheduling hooks and presentation |
| `src/main.rs` | Register the pure AI module |
| `README.md` | Document Human-vs-CPU behavior and verification commands |

### Task 1: Preserve public and blind reservation origins

**Files:**
- Modify: `src/rules/player.rs:9-31`
- Modify: `src/rules/actions.rs:145-239`
- Modify: `src/rules/events.rs:3-18`
- Modify: `src/rules/mod.rs:24-31`
- Modify: `src/battle.rs:1450-1470,2310-2385`
- Test: `src/rules/player.rs`
- Test: `src/rules/actions.rs`

- [ ] **Step 1: Write failing model and rule-event tests**

Add these assertions to the existing test modules:

```rust
#[test]
fn reservation_records_its_visibility_origin() {
    let market = ReservedCard::new(7, ReserveOrigin::Market);
    let blind = ReservedCard::new(8, ReserveOrigin::BlindDeck(CardLevel::Level2));
    assert!(market.is_public());
    assert!(!blind.is_public());
    assert_eq!(blind.card_id, 8);
}

#[test]
fn reserve_actions_emit_the_same_origin_stored_on_player() {
    let mut game = game2();
    let result = apply_action(
        &mut game,
        0,
        PlayerAction::ReserveDeckCard(CardLevel::Level2),
    )
    .unwrap();
    let reserved = game.player(0).reserved_cards[0];
    assert_eq!(reserved.origin, ReserveOrigin::BlindDeck(CardLevel::Level2));
    assert!(result.events.iter().any(|event| matches!(
        event,
        GameEvent::CardReserved {
            origin: ReserveOrigin::BlindDeck(CardLevel::Level2),
            ..
        }
    )));
}
```

- [ ] **Step 2: Run the focused tests to verify the new types are missing**

Run:

```powershell
cargo test reservation_records_its_visibility_origin
cargo test reserve_actions_emit_the_same_origin_stored_on_player
```

Expected: compilation fails because `ReservedCard`, `ReserveOrigin`, and `origin` do not exist.

- [ ] **Step 3: Add the reservation types and migrate `PlayerState`**

Add to `src/rules/player.rs`, import `CardId` and `CardLevel`, and change the vector element type:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ReserveOrigin {
    Market,
    BlindDeck(CardLevel),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReservedCard {
    pub card_id: CardId,
    pub origin: ReserveOrigin,
}

impl ReservedCard {
    pub const fn new(card_id: CardId, origin: ReserveOrigin) -> Self {
        Self { card_id, origin }
    }

    pub const fn is_public(self) -> bool {
        matches!(self.origin, ReserveOrigin::Market)
    }
}

pub struct PlayerState {
    pub id: PlayerId,
    pub tokens: TokenSet,
    pub reserved_cards: Vec<ReservedCard>,
    pub purchased_cards: Vec<CardId>,
    pub nobles: Vec<NobleId>,
}
```

Update existing test setup from raw IDs to `ReservedCard::new(id, origin)`.

- [ ] **Step 4: Migrate actions and events without changing game semantics**

Use the exact origin at both storage and event creation sites:

```rust
let reserved = ReservedCard::new(card.id, ReserveOrigin::Market);
state.player_mut(player).reserved_cards.push(reserved);
events.push(GameEvent::CardReserved {
    player,
    card: card.id,
    origin: reserved.origin,
    got_gold,
});

let reserved = ReservedCard::new(card.id, ReserveOrigin::BlindDeck(*level));
state.player_mut(player).reserved_cards.push(reserved);
events.push(GameEvent::CardReserved {
    player,
    card: card.id,
    origin: reserved.origin,
    got_gold,
});
```

For reserved purchases, retrieve `reserved.card_id` before looking up the card. Change `GameEvent::CardReserved` to:

```rust
CardReserved {
    player: PlayerId,
    card: CardId,
    origin: ReserveOrigin,
    got_gold: bool,
},
```

Re-export the new types:

```rust
pub use player::{PlayerState, ReserveOrigin, ReservedCard};
```

- [ ] **Step 5: Make the existing battle UI compile against `ReservedCard`**

At all reserved-card display sites, replace direct IDs with the struct field:

```rust
for (i, reserved) in p.reserved_cards.iter().copied().enumerate() {
    if let Some(card) = model.0.card_store.get(reserved.card_id) {
        let is_owner = row.0 == model.0.current_id();
        spawn_reserved_card_mini(row_c, *card, row.0, i, is_owner);
    }
}
```

Change event matches from `from_deck` to `origin`. Do not hide card faces yet; Task 11 adds viewer-aware rendering after the AI controller exists.

- [ ] **Step 6: Run formatting and all rule tests**

Run:

```powershell
cargo fmt --check
cargo test rules::
```

Expected: all rule tests pass, including market reserve, blind reserve, buying reserved cards and the smoke game.

- [ ] **Step 7: Commit the model migration**

```powershell
git add src/rules/player.rs src/rules/actions.rs src/rules/events.rs src/rules/mod.rs src/battle.rs
git commit -m "refactor(rules): track reservation visibility"
```

### Task 2: Add one authoritative legal-main-action API

**Files:**
- Modify: `src/rules/actions.rs:16-180`
- Modify: `src/rules/mod.rs:29`
- Test: `src/rules/actions.rs`

- [ ] **Step 1: Write exhaustive legal-action tests**

```rust
#[test]
fn legal_actions_are_unique_and_validate() {
    let game = game2();
    let actions = legal_actions(&game, 0);
    assert!(!actions.is_empty());
    for (index, action) in actions.iter().enumerate() {
        assert!(validate_action(&game, 0, action).is_ok(), "{action:?}");
        assert!(!actions[..index].contains(action), "duplicate {action:?}");
    }
}

#[test]
fn initial_game_lists_token_and_reserve_choices() {
    let game = game2();
    let actions = legal_actions(&game, 0);
    assert!(actions.contains(&PlayerAction::TakeThreeDifferentTokens(vec![
        GemColor::White,
        GemColor::Blue,
        GemColor::Green,
    ])));
    assert!(actions.contains(&PlayerAction::TakeTwoSameTokens(GemColor::Red)));
    assert!(actions.contains(&PlayerAction::ReserveVisibleCard {
        level: CardLevel::Level3,
        idx: 3,
    }));
    assert!(actions.contains(&PlayerAction::ReserveDeckCard(CardLevel::Level2)));
}
```

- [ ] **Step 2: Verify the tests fail on the absent API**

Run:

```powershell
cargo test legal_actions_are_unique_and_validate
cargo test initial_game_lists_token_and_reserve_choices
```

Expected: compilation fails with `cannot find function legal_actions`.

- [ ] **Step 3: Implement exhaustive candidate generation followed by rule validation**

Add this public function to `src/rules/actions.rs`:

```rust
pub fn legal_actions(state: &GameState, player: PlayerId) -> Vec<PlayerAction> {
    if state.is_over() || state.current_id() != player {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for a in 0..GemColor::NORMAL.len() {
        for b in (a + 1)..GemColor::NORMAL.len() {
            for c in (b + 1)..GemColor::NORMAL.len() {
                candidates.push(PlayerAction::TakeThreeDifferentTokens(vec![
                    GemColor::NORMAL[a],
                    GemColor::NORMAL[b],
                    GemColor::NORMAL[c],
                ]));
            }
        }
    }
    for color in GemColor::NORMAL {
        candidates.push(PlayerAction::TakeTwoSameTokens(color));
    }
    for level in CardLevel::ALL {
        for idx in 0..state.market.visible(level).len() {
            candidates.push(PlayerAction::ReserveVisibleCard { level, idx });
            candidates.push(PlayerAction::BuyVisibleCard { level, idx });
        }
        candidates.push(PlayerAction::ReserveDeckCard(level));
    }
    for idx in 0..state.player(player).reserved_cards.len() {
        candidates.push(PlayerAction::BuyReservedCard(idx));
    }

    candidates
        .into_iter()
        .filter(|action| validate_action(state, player, action).is_ok())
        .collect()
}
```

Re-export it from `src/rules/mod.rs` beside `apply_action` and `validate_action`.

- [ ] **Step 4: Run the action and complete rule suites**

```powershell
cargo test rules::actions::tests
cargo test rules::
```

Expected: both commands pass; generated actions contain no invalid or duplicate entries.

- [ ] **Step 5: Commit the legal-action seam**

```powershell
git add src/rules/actions.rs src/rules/mod.rs
git commit -m "feat(rules): enumerate legal player actions"
```

### Task 3: Model AI decisions and complete-turn simulation

**Files:**
- Create: `src/ai/mod.rs`
- Create: `src/ai/decision.rs`
- Modify: `src/rules/actions.rs:16`
- Modify: `src/rules/token.rs:5`
- Modify: `src/main.rs:12-15`
- Test: `src/ai/decision.rs`

- [ ] **Step 1: Register an empty AI module and write decision tests**

Add `mod ai;` beside `mod battle;` in `src/main.rs`. Create `src/ai/mod.rs` with `mod decision;` and these tests in `decision.rs`:

```rust
#[test]
fn discard_decisions_return_exactly_the_excess() {
    let mut game = GameState::new_seeded(2, 7).unwrap();
    game.players[0].tokens = TokenSet {
        white: 4,
        blue: 4,
        green: 4,
        ..Default::default()
    };
    let sim = SimulationState::new(game, DecisionContext::Discard { excess: 2 });
    let decisions = sim.legal_decisions().unwrap();
    assert!(decisions.iter().all(|decision| match decision {
        AiDecision::Discard(tokens) => tokens.total() == 2,
        _ => false,
    }));
}

#[test]
fn applying_main_decision_updates_the_pending_context() {
    let mut game = GameState::new_seeded(2, 11).unwrap();
    game.players[0].tokens = TokenSet {
        white: 3,
        blue: 3,
        green: 3,
        ..Default::default()
    };
    let mut sim = SimulationState::new(game, DecisionContext::MainTurn);
    sim.apply_decision(AiDecision::Action(
        PlayerAction::TakeThreeDifferentTokens(vec![
            GemColor::White,
            GemColor::Blue,
            GemColor::Green,
        ]),
    ))
    .unwrap();
    assert_eq!(sim.context, DecisionContext::Discard { excess: 2 });
}
```

- [ ] **Step 2: Verify the module fails to compile before its types exist**

Run:

```powershell
cargo test ai::decision::tests
```

Expected: compilation fails for missing `SimulationState`, `DecisionContext`, and `AiDecision`.

- [ ] **Step 3: Add hashable decision types and a shared AI error**

Derive `Hash` for `PlayerAction` and `TokenSet`, then define:

```rust
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DecisionContext {
    MainTurn,
    Discard { excess: u8 },
    ChooseNoble { candidates: Vec<NobleId> },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AiDecision {
    Action(PlayerAction),
    Discard(TokenSet),
    ChooseNoble(NobleId),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DecisionContextKind {
    MainTurn,
    Discard,
    ChooseNoble,
}

impl DecisionContext {
    pub fn kind(&self) -> DecisionContextKind {
        match self {
            Self::MainTurn => DecisionContextKind::MainTurn,
            Self::Discard { .. } => DecisionContextKind::Discard,
            Self::ChooseNoble { .. } => DecisionContextKind::ChooseNoble,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AiError {
    NoLegalDecision,
    InvalidObservation(&'static str),
    Rule(RuleError),
    Cancelled,
    UnsupportedCombinedOutcome,
}

impl From<RuleError> for AiError {
    fn from(value: RuleError) -> Self {
        Self::Rule(value)
    }
}
```

Re-export these types from `src/ai/mod.rs`.

- [ ] **Step 4: Implement legal follow-up enumeration and rule-backed state transitions**

`SimulationState` must contain only `GameState` and the current decision context:

```rust
#[derive(Clone, Debug)]
pub struct SimulationState {
    pub game: GameState,
    pub context: DecisionContext,
}

impl SimulationState {
    pub fn new(game: GameState, context: DecisionContext) -> Self {
        Self { game, context }
    }

    pub fn legal_decisions(&self) -> Result<Vec<AiDecision>, AiError> {
        let player = self.game.current_id();
        match &self.context {
            DecisionContext::MainTurn => Ok(legal_actions(&self.game, player)
                .into_iter()
                .map(AiDecision::Action)
                .collect()),
            DecisionContext::Discard { excess } => Ok(enumerate_discards(
                self.game.player(player).tokens,
                *excess,
            )
            .into_iter()
            .map(AiDecision::Discard)
            .collect()),
            DecisionContext::ChooseNoble { candidates } => Ok(candidates
                .iter()
                .copied()
                .map(AiDecision::ChooseNoble)
                .collect()),
        }
    }

    pub fn apply_decision(&mut self, decision: AiDecision) -> Result<Vec<GameEvent>, AiError> {
        let player = self.game.current_id();
        let result = match (&self.context, decision) {
            (DecisionContext::MainTurn, AiDecision::Action(action)) => {
                apply_action(&mut self.game, player, action)?
            }
            (DecisionContext::Discard { .. }, AiDecision::Discard(tokens)) => {
                resume(&mut self.game, player, Resume::DiscardTokens(tokens))?
            }
            (DecisionContext::ChooseNoble { .. }, AiDecision::ChooseNoble(noble)) => {
                resume(&mut self.game, player, Resume::ChooseNoble(noble))?
            }
            _ => return Err(AiError::NoLegalDecision),
        };
        self.context = match result.outcome {
            ActionOutcome::Complete => DecisionContext::MainTurn,
            ActionOutcome::NeedDiscardTokens { excess } => DecisionContext::Discard { excess },
            ActionOutcome::NeedChooseNoble { candidates } => {
                DecisionContext::ChooseNoble { candidates }
            }
            ActionOutcome::NeedFinalDiscardThenChooseNoble { .. } => {
                return Err(AiError::UnsupportedCombinedOutcome)
            }
        };
        Ok(result.events)
    }
}
```

Implement `enumerate_discards` by recursively assigning `0..=min(have, remaining)` for `[White, Blue, Green, Red, Black, Gold]`, pushing a `TokenSet` only when the final remaining amount is zero. Sort output by the six token counts and deduplicate it.

- [ ] **Step 5: Run decision and full tests**

```powershell
cargo fmt --check
cargo test ai::decision::tests
cargo test
```

Expected: decision tests and the existing game tests pass.

- [ ] **Step 6: Commit the AI decision model**

```powershell
git add src/ai/mod.rs src/ai/decision.rs src/rules/actions.rs src/rules/token.rs src/main.rs
git commit -m "feat(ai): model complete rule decisions"
```

### Task 4: Build redacted per-player observations and information-set keys

**Files:**
- Create: `src/ai/observation.rs`
- Modify: `src/ai/mod.rs`
- Modify: `src/rules/token.rs:5`
- Test: `src/ai/observation.rs`

- [ ] **Step 1: Write paired hidden-information tests**

```rust
#[test]
fn opponent_blind_card_is_redacted_but_own_card_is_known() {
    let mut game = GameState::new_seeded(2, 17).unwrap();
    let blind = game.decks.pop(CardLevel::Level1).unwrap();
    game.players[1].reserved_cards.push(ReservedCard::new(
        blind.id,
        ReserveOrigin::BlindDeck(CardLevel::Level1),
    ));

    let human = AiObservation::from_game(&game, 0);
    let cpu = AiObservation::from_game(&game, 1);
    assert_eq!(
        human.players[1].reserved[0],
        ObservedReservation::HiddenBlind(CardLevel::Level1)
    );
    assert_eq!(
        cpu.players[1].reserved[0],
        ObservedReservation::Known(ReservedCard::new(
            blind.id,
            ReserveOrigin::BlindDeck(CardLevel::Level1),
        ))
    );
}

#[test]
fn changing_an_unseen_card_does_not_change_the_information_set_key() {
    let mut left = GameState::new_seeded(2, 21).unwrap();
    let mut right = left.clone();
    let left_card = left.decks.level1.pop().unwrap();
    let right_card = right.decks.level1.remove(0);
    left.players[1].reserved_cards.push(ReservedCard::new(
        left_card.id,
        ReserveOrigin::BlindDeck(CardLevel::Level1),
    ));
    right.players[1].reserved_cards.push(ReservedCard::new(
        right_card.id,
        ReserveOrigin::BlindDeck(CardLevel::Level1),
    ));
    let context = DecisionContext::MainTurn;
    assert_eq!(
        AiObservation::from_game(&left, 0).information_set_key(&context),
        AiObservation::from_game(&right, 0).information_set_key(&context),
    );
}
```

- [ ] **Step 2: Verify observation tests fail before the API exists**

```powershell
cargo test ai::observation::tests
```

Expected: compilation fails for missing observation types.

- [ ] **Step 3: Define an observation that contains no inaccessible `CardId`**

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ObservedReservation {
    Known(ReservedCard),
    HiddenBlind(CardLevel),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ObservedPlayer {
    pub id: PlayerId,
    pub tokens: TokenSet,
    pub reserved: Vec<ObservedReservation>,
    pub purchased_cards: Vec<CardId>,
    pub nobles: Vec<NobleId>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AiObservation {
    pub observer: PlayerId,
    pub players: Vec<ObservedPlayer>,
    pub bank: TokenSet,
    pub market: [Vec<DevelopmentCard>; 3],
    pub deck_remaining: [usize; 3],
    pub nobles_available: Vec<Noble>,
    pub nobles_taken: Vec<NobleId>,
    pub current_player: PlayerId,
    pub round_start_player: PlayerId,
    pub end_triggered: bool,
    pub winner: Option<PlayerId>,
    pub final_player: Option<PlayerId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InfoSetKey(pub u64);
```

Derive `Hash` for `TokenSet`. In `from_game`, emit `Known` when `owner == observer` or `reserved.is_public()`, otherwise emit only `HiddenBlind(level)`.

- [ ] **Step 4: Hash exactly the fields visible to the observer**

Implement `information_set_key` with `std::collections::hash_map::DefaultHasher`. Hash `observer`, all `ObservedPlayer` values, bank, each market row, deck counts, available/taken nobles, turn/terminal fields and `DecisionContext`. Do not access `GameState` or a hidden card ID in this method.

```rust
pub fn information_set_key(&self, context: &DecisionContext) -> InfoSetKey {
    let mut hasher = DefaultHasher::new();
    self.observer.hash(&mut hasher);
    self.players.hash(&mut hasher);
    self.bank.hash(&mut hasher);
    self.market.hash(&mut hasher);
    self.deck_remaining.hash(&mut hasher);
    self.nobles_available.hash(&mut hasher);
    self.nobles_taken.hash(&mut hasher);
    self.current_player.hash(&mut hasher);
    self.round_start_player.hash(&mut hasher);
    self.end_triggered.hash(&mut hasher);
    self.winner.hash(&mut hasher);
    self.final_player.hash(&mut hasher);
    context.hash(&mut hasher);
    InfoSetKey(hasher.finish())
}
```

- [ ] **Step 5: Run the observation tests and leak scan**

```powershell
cargo test ai::observation::tests
rg -n "GameState|decks\.level|HiddenBlind\([^)]*CardId" src/ai/observation.rs
```

Expected: tests pass; `GameState` appears only in the `from_game` constructor signature/body, and `HiddenBlind` contains only `CardLevel`.

- [ ] **Step 6: Commit the observation boundary**

```powershell
git add src/ai/observation.rs src/ai/mod.rs src/rules/token.rs
git commit -m "feat(ai): redact hidden game information"
```

### Task 5: Determinize observations and re-determinize for each actor

**Files:**
- Create: `src/ai/determinization.rs`
- Modify: `src/ai/mod.rs`
- Modify: `src/rules/mod.rs:20-30`
- Test: `src/ai/determinization.rs`

- [ ] **Step 1: Write card-conservation and repeat-sampling tests**

```rust
#[test]
fn determinization_preserves_every_card_exactly_once() {
    let game = game_with_opponent_blind_reservation();
    let observation = AiObservation::from_game(&game, 0);
    let mut rng = StdRng::seed_from_u64(31);
    let simulation = determinize(&observation, DecisionContext::MainTurn, &mut rng).unwrap();
    let ids = all_located_card_ids(&simulation.game);
    assert_eq!(ids.len(), 90);
    let unique: HashSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), 90);
    assert_eq!(simulation.game.decks.remaining(CardLevel::Level1), observation.deck_remaining[0]);
}

#[test]
fn different_seeds_change_hidden_world_but_not_public_state() {
    let game = game_with_opponent_blind_reservation();
    let observation = AiObservation::from_game(&game, 0);
    let mut a = StdRng::seed_from_u64(1);
    let mut b = StdRng::seed_from_u64(2);
    let left = determinize(&observation, DecisionContext::MainTurn, &mut a).unwrap();
    let right = determinize(&observation, DecisionContext::MainTurn, &mut b).unwrap();
    assert_ne!(left.game.decks.level1, right.game.decks.level1);
    assert_eq!(left.game.market.level1_visible, right.game.market.level1_visible);
    assert_eq!(left.game.bank, right.game.bank);
}

fn game_with_opponent_blind_reservation() -> GameState {
    let mut game = GameState::new_seeded(2, 29).unwrap();
    let card = game.decks.pop(CardLevel::Level1).unwrap();
    game.players[1].reserved_cards.push(ReservedCard::new(
        card.id,
        ReserveOrigin::BlindDeck(CardLevel::Level1),
    ));
    game
}

fn all_located_card_ids(game: &GameState) -> Vec<CardId> {
    let mut ids = Vec::new();
    for level in CardLevel::ALL {
        ids.extend(game.market.visible(level).iter().map(|card| card.id));
        ids.extend(game.decks.deck(level).iter().map(|card| card.id));
    }
    for player in &game.players {
        ids.extend(player.reserved_cards.iter().map(|card| card.card_id));
        ids.extend(player.purchased_cards.iter().copied());
    }
    ids
}
```

- [ ] **Step 2: Verify determinization tests fail on the missing module**

```powershell
cargo test ai::determinization::tests
```

Expected: compilation fails because `determinize` is missing.

- [ ] **Step 3: Re-export standard static data required to rebuild a state**

In `src/rules/mod.rs` re-export:

```rust
pub use card::{
    standard_deck, CardBonus, CardId, CardLevel, CardStore, DevelopmentCard, GemCost,
};
pub use noble::{standard_nobles, Noble, NobleBoard, NobleId, NobleStore};
```

- [ ] **Step 4: Implement no-replacement hidden-card allocation**

Implement this sequence in `determinize`:

```rust
pub fn determinize<R: Rng + ?Sized>(
    observation: &AiObservation,
    context: DecisionContext,
    rng: &mut R,
) -> Result<SimulationState, AiError> {
    let standard = standard_deck();
    let mut known = HashSet::new();
    for row in &observation.market {
        known.extend(row.iter().map(|card| card.id));
    }
    for player in &observation.players {
        known.extend(player.purchased_cards.iter().copied());
        known.extend(player.reserved.iter().filter_map(|reserved| match reserved {
            ObservedReservation::Known(card) => Some(card.card_id),
            ObservedReservation::HiddenBlind(_) => None,
        }));
    }

    let mut candidates: [Vec<DevelopmentCard>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for card in standard.iter().copied().filter(|card| !known.contains(&card.id)) {
        candidates[level_index(card.level)].push(card);
    }
    for cards in &mut candidates {
        cards.shuffle(rng);
    }

    let players = materialize_players(&observation.players, &mut candidates)?;
    let decks = materialize_decks(observation.deck_remaining, &mut candidates)?;
    if candidates.iter().any(|cards| !cards.is_empty()) {
        return Err(AiError::InvalidObservation("unallocated cards remain"));
    }

    let card_store = CardStore::from_cards(&standard);
    let nobles = standard_nobles();
    let game = GameState {
        players,
        bank: Bank { tokens: observation.bank },
        decks,
        market: materialize_market(&observation.market),
        nobles: NobleBoard {
            available: observation.nobles_available.clone(),
            taken: observation.nobles_taken.clone(),
        },
        card_store,
        noble_store: NobleStore::from_nobles(&nobles),
        current_player: observation.current_player,
        round_start_player: observation.round_start_player,
        end_triggered: observation.end_triggered,
        winner: observation.winner,
        final_player: observation.final_player,
    };
    Ok(SimulationState::new(game, context))
}
```

`materialize_players` must replace every `HiddenBlind(level)` with one `ReservedCard` popped from the same-level candidate vector. `materialize_decks` must take exactly each observed count and fail if insufficient. `materialize_market` copies the three visible rows into `Market`.

- [ ] **Step 5: Add actor-relative private-knowledge preservation**

Define a ledger that remembers cards known to their owner within an iteration:

```rust
#[derive(Clone, Debug)]
pub struct PrivateKnowledge {
    blind_by_player: Vec<Vec<ReservedCard>>,
}

impl PrivateKnowledge {
    pub fn from_state(state: &GameState) -> Self {
        let blind_by_player = state
            .players
            .iter()
            .map(|player| {
                player
                    .reserved_cards
                    .iter()
                    .copied()
                    .filter(|card| matches!(card.origin, ReserveOrigin::BlindDeck(_)))
                    .collect()
            })
            .collect();
        Self { blind_by_player }
    }
}
```

Add `redeterminize_for_actor(simulation, knowledge, actor, rng)`: create the actor's redacted observation from the current public state, replace that actor's `Known` blind entries with the ledger entries in slot order, call `determinize`, and copy the new `GameState` back while preserving `simulation.context`. When an actor blind-reserves or buys a blind reserved card, update only that actor's ledger. This ensures a later actor-relative sample cannot erase private knowledge held by its owner.

- [ ] **Step 6: Test actor-relative information safety**

Add a test that re-determinizes for player 0, then player 1, then player 0 again. Assert player 0's own blind reservation is restored to its original ID, while player 1's unseen card may change and all 90 card IDs remain unique.

Run:

```powershell
cargo test ai::determinization::tests
cargo test
```

Expected: all determinization invariants and the full suite pass.

- [ ] **Step 7: Commit determinization**

```powershell
git add src/ai/determinization.rs src/ai/mod.rs src/rules/mod.rs
git commit -m "feat(ai): sample observer-consistent game states"
```

### Task 6: Implement the six-feature cutoff evaluator

**Files:**
- Create: `src/ai/evaluation.rs`
- Modify: `src/ai/mod.rs`
- Test: `src/ai/evaluation.rs`

- [ ] **Step 1: Write evaluator boundary and preference tests**

```rust
#[test]
fn terminal_result_is_binary_from_root_perspective() {
    let mut game = GameState::new_seeded(2, 41).unwrap();
    game.winner = Some(1);
    assert_eq!(evaluate(&game, 1), 1.0);
    assert_eq!(evaluate(&game, 0), 0.0);
}

#[test]
fn score_and_engine_advantage_raise_cutoff_value() {
    let base = GameState::new_seeded(2, 43).unwrap();
    let mut improved = base.clone();
    let card = improved.market.level1_visible.remove(0);
    improved.players[0].purchased_cards.push(card.id);
    assert!(evaluate(&improved, 0) > evaluate(&base, 0));
}

#[test]
fn cutoff_value_is_always_a_probability() {
    for seed in 0..32 {
        let game = GameState::new_seeded(2, seed).unwrap();
        assert!((0.0..=1.0).contains(&evaluate(&game, 0)));
    }
}
```

- [ ] **Step 2: Verify evaluator tests fail before implementation**

```powershell
cargo test ai::evaluation::tests
```

Expected: compilation fails because `evaluate` is missing.

- [ ] **Step 3: Add explicit weights and normalized feature helpers**

```rust
#[derive(Clone, Copy, Debug)]
pub struct EvaluationWeights {
    pub score: f32,
    pub engine: f32,
    pub nobles: f32,
    pub buying_power: f32,
    pub token_efficiency: f32,
    pub reservations: f32,
}

impl Default for EvaluationWeights {
    fn default() -> Self {
        Self {
            score: 0.35,
            engine: 0.20,
            nobles: 0.15,
            buying_power: 0.15,
            token_efficiency: 0.10,
            reservations: 0.05,
        }
    }
}

fn normalized_difference(left: f32, right: f32, scale: f32) -> f32 {
    ((left - right) / scale).clamp(-1.0, 1.0)
}
```

Implement `score_feature` using score difference divided by 15, `engine_feature` using total bonuses plus `0.2` per distinct covered color divided by 15, and `noble_feature` using the best satisfied-requirement ratio among available nobles.

- [ ] **Step 4: Implement buying, token and reservation features**

Use these exact utility definitions:

```text
card utility = prestige * 2 + 1 permanent bonus + noble-progress delta
buying power = maximum utility among currently affordable market/reserved cards
token efficiency = useful tokens for the cheapest visible or owned-reserved deficit / max(token_total, 1)
reservation quality = maximum reserved-card utility / 12 - 0.15 when all three slots are filled with zero-prestige cards
```

Compute each root-minus-opponent difference with `normalized_difference`. Implement the final function:

```rust
pub fn evaluate(game: &GameState, root: PlayerId) -> f32 {
    if let Some(winner) = game.winner {
        return if winner == root { 1.0 } else { 0.0 };
    }
    let opponent = if root == 0 { 1 } else { 0 };
    let weights = EvaluationWeights::default();
    let advantage = weights.score * score_feature(game, root, opponent)
        + weights.engine * engine_feature(game, root, opponent)
        + weights.nobles * noble_feature(game, root, opponent)
        + weights.buying_power * buying_power_feature(game, root, opponent)
        + weights.token_efficiency * token_efficiency_feature(game, root, opponent)
        + weights.reservations * reservation_feature(game, root, opponent);
    0.5 + 0.5 * advantage.clamp(-1.0, 1.0)
}
```

- [ ] **Step 5: Run evaluator and full tests**

```powershell
cargo test ai::evaluation::tests
cargo test
```

Expected: all tests pass and every nonterminal value remains in `[0, 1]`.

- [ ] **Step 6: Commit evaluation**

```powershell
git add src/ai/evaluation.rs src/ai/mod.rs
git commit -m "feat(ai): evaluate nonterminal positions"
```

### Task 7: Add heuristic rollout and a guaranteed legal fallback

**Files:**
- Create: `src/ai/rollout.rs`
- Modify: `src/ai/mod.rs`
- Test: `src/ai/rollout.rs`

- [ ] **Step 1: Write fallback and rollout-limit tests**

```rust
#[test]
fn fallback_is_always_legal() {
    let game = GameState::new_seeded(2, 47).unwrap();
    let sim = SimulationState::new(game, DecisionContext::MainTurn);
    let decision = fallback_decision(&sim).unwrap();
    assert!(sim.legal_decisions().unwrap().contains(&decision));
}

#[test]
fn rollout_stops_at_the_complete_turn_limit() {
    let game = GameState::new_seeded(2, 53).unwrap();
    let mut sim = SimulationState::new(game, DecisionContext::MainTurn);
    let mut rng = StdRng::seed_from_u64(59);
    let result = rollout(&mut sim, 0, 3, 0.15, &mut rng, || false).unwrap();
    assert!(result.complete_turns <= 3);
    assert!((0.0..=1.0).contains(&result.reward));
}
```

- [ ] **Step 2: Verify rollout tests fail**

```powershell
cargo test ai::rollout::tests
```

Expected: compilation fails for missing rollout functions.

- [ ] **Step 3: Implement observation-safe decision weights**

For each legal decision, clone the simulation, apply the decision, and evaluate from the acting player's perspective. Use:

```rust
fn decision_weight(state: &SimulationState, decision: &AiDecision) -> f32 {
    let actor = state.game.current_id();
    let before = evaluate(&state.game, actor);
    let mut after = state.clone();
    if after.apply_decision(decision.clone()).is_err() {
        return 0.01;
    }
    let gain = evaluate(&after.game, actor) - before;
    (gain * 4.0).exp().max(0.01)
}
```

`fallback_decision` returns the legal decision with greatest weight; ties use `format!("{decision:?}")` lexical order for deterministic behavior.

- [ ] **Step 4: Implement epsilon-weighted rollout**

```rust
pub struct RolloutResult {
    pub reward: f32,
    pub complete_turns: u16,
}

pub fn rollout<R: Rng + ?Sized>(
    state: &mut SimulationState,
    root: PlayerId,
    max_complete_turns: u16,
    epsilon: f32,
    rng: &mut R,
    should_stop: impl Fn() -> bool,
) -> Result<RolloutResult, AiError> {
    let mut complete_turns = 0;
    while !state.game.is_over() && complete_turns < max_complete_turns {
        if should_stop() {
            return Err(AiError::Cancelled);
        }
        let before_player = state.game.current_id();
        let decisions = state.legal_decisions()?;
        if decisions.is_empty() {
            return Err(AiError::NoLegalDecision);
        }
        let decision = if rng.random::<f32>() < epsilon {
            decisions[rng.random_range(0..decisions.len())].clone()
        } else {
            weighted_choice(state, &decisions, rng)
        };
        state.apply_decision(decision)?;
        if matches!(state.context, DecisionContext::MainTurn)
            && state.game.current_id() != before_player
        {
            complete_turns += 1;
        }
    }
    Ok(RolloutResult {
        reward: evaluate(&state.game, root),
        complete_turns,
    })
}
```

`weighted_choice` sums positive `decision_weight` values, draws once in `[0,total)`, and returns the first action whose cumulative weight crosses the draw.

- [ ] **Step 5: Run rollout and full suites**

```powershell
cargo test ai::rollout::tests
cargo test
```

Expected: rollout respects the turn cap and always returns a probability or a typed cancellation error.

- [ ] **Step 6: Commit rollout policy**

```powershell
git add src/ai/rollout.rs src/ai/mod.rs
git commit -m "feat(ai): add heuristic rollout policy"
```

### Task 8: Implement cancellable MO-ISMCTS

**Files:**
- Create: `src/ai/mcts.rs`
- Modify: `src/ai/mod.rs`
- Test: `src/ai/mcts.rs`

- [ ] **Step 1: Write UCT, root-choice and deterministic-search tests**

```rust
#[test]
fn opponent_uct_inverts_root_reward() {
    let high_for_root = EdgeStats { visits: 10, total_reward: 9.0, availability: 20 };
    let low_for_root = EdgeStats { visits: 10, total_reward: 2.0, availability: 20 };
    assert!(uct_score(&high_for_root, true, 2.0_f32.sqrt())
        > uct_score(&low_for_root, true, 2.0_f32.sqrt()));
    assert!(uct_score(&high_for_root, false, 2.0_f32.sqrt())
        < uct_score(&low_for_root, false, 2.0_f32.sqrt()));
}

#[test]
fn root_choice_prefers_visits_before_mean_reward() {
    let a = RootActionStat::test_action(0, 20, 0.55);
    let b = RootActionStat::test_action(1, 10, 0.95);
    assert_eq!(choose_root_action(&[a.clone(), b]), Some(a.decision));
}

#[test]
fn fixed_seed_and_iteration_limit_are_deterministic() {
    let game = GameState::new_seeded(2, 61).unwrap();
    let observation = AiObservation::from_game(&game, 0);
    let config = MctsConfig::for_iterations(128);
    let left = search(observation.clone(), DecisionContext::MainTurn, 0, 67, config.clone(), SearchControl::new()).unwrap();
    let right = search(observation, DecisionContext::MainTurn, 0, 67, config, SearchControl::new()).unwrap();
    assert_eq!(left.decision, right.decision);
    assert_eq!(left.metrics.iterations, 128);
}
```

- [ ] **Step 2: Verify MCTS tests fail before implementation**

```powershell
cargo test ai::mcts::tests
```

Expected: compilation fails for missing MCTS types and functions.

- [ ] **Step 3: Define config, cancellation, tree and result types**

```rust
#[derive(Clone, Debug)]
pub struct MctsConfig {
    pub time_limit: Duration,
    pub iteration_limit: Option<u32>,
    pub max_nodes: usize,
    pub max_rollout_turns: u16,
    pub exploration: f32,
    pub rollout_epsilon: f32,
}

impl MctsConfig {
    pub fn normal() -> Self {
        Self {
            time_limit: Duration::from_secs(1),
            iteration_limit: None,
            max_nodes: 100_000,
            max_rollout_turns: 60,
            exploration: 2.0_f32.sqrt(),
            rollout_epsilon: 0.15,
        }
    }

    pub fn for_iterations(iterations: u32) -> Self {
        Self { iteration_limit: Some(iterations), time_limit: Duration::MAX, ..Self::normal() }
    }
}

#[derive(Clone, Default)]
pub struct SearchControl(Arc<AtomicBool>);

impl SearchControl {
    pub fn new() -> Self { Self::default() }
    pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed); }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}

#[derive(Clone, Debug, Default)]
struct EdgeStats {
    visits: u32,
    total_reward: f32,
    availability: u32,
}

#[derive(Clone, Debug, Default)]
struct Node {
    visits: u32,
    edges: HashMap<AiDecision, EdgeStats>,
}

pub struct AiSearchMetrics {
    pub elapsed: Duration,
    pub iterations: u32,
    pub nodes: usize,
    pub used_fallback: bool,
    pub root_actions: Vec<RootActionStat>,
}

#[derive(Clone, Debug)]
pub struct RootActionStat {
    pub decision: AiDecision,
    pub visits: u32,
    pub mean_reward: f32,
}

#[cfg(test)]
impl RootActionStat {
    fn test_action(index: usize, visits: u32, mean_reward: f32) -> Self {
        Self {
            decision: AiDecision::Action(PlayerAction::BuyReservedCard(index)),
            visits,
            mean_reward,
        }
    }
}

pub struct AiSearchResult {
    pub decision: AiDecision,
    pub seed: u64,
    pub metrics: AiSearchMetrics,
}
```

- [ ] **Step 4: Implement availability-aware UCT and stable expansion**

For every visit to an information set, increment `availability` for every legal edge. Expand one currently legal action absent from the node, ordered by descending rollout `decision_weight` then debug-string order. When all legal actions exist, select maximum:

```rust
fn uct_score(edge: &EdgeStats, maximizing_root: bool, exploration: f32) -> f32 {
    if edge.visits == 0 {
        return f32::INFINITY;
    }
    let mean = edge.total_reward / edge.visits as f32;
    let exploitation = if maximizing_root { mean } else { 1.0 - mean };
    let available = edge.availability.max(1) as f32;
    exploitation + exploration * (available.ln() / edge.visits as f32).sqrt()
}
```

Track the visited `(InfoSetKey, AiDecision)` edges in path order. Backpropagate the same root-player reward to each edge and increment its node visit count.

- [ ] **Step 5: Implement the bounded search loop**

The loop must use a fresh root determinization on every iteration and actor-relative re-determinization before every tree decision:

```rust
pub fn search(
    observation: AiObservation,
    context: DecisionContext,
    root: PlayerId,
    seed: u64,
    config: MctsConfig,
    control: SearchControl,
) -> Result<AiSearchResult, AiError> {
    let started = Instant::now();
    let deadline = started.checked_add(config.time_limit);
    let mut rng = StdRng::seed_from_u64(seed);
    let root_sim = determinize(&observation, context.clone(), &mut rng)?;
    let fallback = fallback_decision(&root_sim)?;
    let mut tree = HashMap::<InfoSetKey, Node>::new();
    let mut iterations = 0;

    while config.iteration_limit.map_or(true, |limit| iterations < limit)
        && deadline.map_or(true, |limit| Instant::now() < limit)
        && !control.is_cancelled()
    {
        let mut simulation = determinize(&observation, context.clone(), &mut rng)?;
        let mut knowledge = PrivateKnowledge::from_state(&simulation.game);
        let mut path = Vec::new();
        let iteration = tree_iteration(
            &mut tree,
            &mut simulation,
            &mut knowledge,
            root,
            &config,
            &control,
            deadline,
            &mut rng,
            &mut path,
        );
        match iteration {
            Ok(()) => iterations += 1,
            Err(AiError::Cancelled) if control.is_cancelled() => {
                return Err(AiError::Cancelled);
            }
            Err(AiError::Cancelled) => break,
            Err(error) => return Err(error),
        }
    }

    if control.is_cancelled() {
        return Err(AiError::Cancelled);
    }
    let root_key = observation.information_set_key(&context);
    let root_stats = collect_root_stats(tree.get(&root_key));
    let selected = choose_root_action(&root_stats).unwrap_or_else(|| fallback.clone());
    Ok(AiSearchResult {
        decision: selected,
        seed,
        metrics: AiSearchMetrics {
            elapsed: started.elapsed(),
            iterations,
            nodes: tree.len(),
            used_fallback: root_stats.is_empty(),
            root_actions: root_stats,
        },
    })
}
```

`tree_iteration` accepts `deadline: Option<Instant>`, stops tree descent after one new edge is expanded, calls `rollout`, and backpropagates its reward. If `tree.len() == max_nodes`, it selects only existing legal edges and does not insert a node. Its rollout stop closure returns true when `control.is_cancelled()` or `deadline.is_some_and(|limit| Instant::now() >= limit)`, so time and cancellation are checked at least once per simulated decision. `rollout` reports that stop as `AiError::Cancelled`; the outer match above distinguishes a real atomic cancellation from a wall-clock deadline. A deadline breaks the loop and returns the best root action accumulated so far.

- [ ] **Step 6: Implement stable root selection and metrics**

Sort `RootActionStat` by descending visits, descending mean reward, then ascending debug-string action representation. `choose_root_action` returns the first decision. Log no information from hidden determinized states.

- [ ] **Step 7: Run MCTS tests, including node-limit and cancellation cases**

Add tests with `max_nodes = 1` and with a pre-cancelled `SearchControl`; assert the former returns a legal fallback/search result and the latter returns `AiError::Cancelled`.

Run:

```powershell
cargo test ai::mcts::tests
cargo test
```

Expected: fixed-iteration tests are deterministic; cancellation and node limits do not panic.

- [ ] **Step 8: Commit MO-ISMCTS**

```powershell
git add src/ai/mcts.rs src/ai/mod.rs
git commit -m "feat(ai): implement cancellable MO-ISMCTS"
```

### Task 9: Centralize Battle rule-decision submission

**Files:**
- Modify: `src/battle.rs:30-60,106-132,307-310,1293-1410,1979-2068`
- Test: `src/battle.rs`

- [ ] **Step 1: Write routing tests before refactoring systems**

```rust
#[test]
fn queued_ai_and_human_decisions_share_rule_commands() {
    assert!(matches!(
        QueuedRuleDecision::Action(PlayerAction::TakeTwoSameTokens(GemColor::Red)),
        QueuedRuleDecision::Action(_),
    ));
    assert!(matches!(
        QueuedRuleDecision::Resume(Resume::ChooseNoble(2)),
        QueuedRuleDecision::Resume(_),
    ));
}

#[test]
fn routed_outcome_preserves_pending_choice() {
    let route = route_outcome(ActionOutcome::NeedDiscardTokens { excess: 2 });
    assert_eq!(route, Some(BattlePhase::AwaitDiscard { excess: 2 }));
}
```

- [ ] **Step 2: Verify the routing test fails on the old `ActionQueue`**

```powershell
cargo test queued_ai_and_human_decisions_share_rule_commands
cargo test routed_outcome_preserves_pending_choice
```

Expected: compilation fails because `QueuedRuleDecision` does not exist.

- [ ] **Step 3: Replace action-only queueing with a rule-decision queue**

```rust
#[derive(Clone, Debug)]
enum QueuedRuleDecision {
    Action(PlayerAction),
    Resume(Resume),
}

#[derive(Resource, Default)]
struct RuleDecisionQueue(Vec<QueuedRuleDecision>);

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
struct BattleRevision(u64);
```

Mouse and keyboard systems push `QueuedRuleDecision::Action`. Discard and noble overlay systems push `QueuedRuleDecision::Resume` after validating their local UI selection, then despawn their overlay; they no longer call `resume` directly.

- [ ] **Step 4: Implement one state-mutating apply system**

Replace `apply_actions` with `apply_rule_decisions`. For each queued item, read `pid = model.0.current_id()`, call the matching rule API, and route `ActionResult` through one helper:

```rust
fn apply_queued_decision(
    state: &mut GameState,
    player: PlayerId,
    decision: QueuedRuleDecision,
) -> Result<ActionResult, RuleError> {
    match decision {
        QueuedRuleDecision::Action(action) => apply_action(state, player, action),
        QueuedRuleDecision::Resume(resume_decision) => resume(state, player, resume_decision),
    }
}
```

On success: extend pending events, route `ActionOutcome`, increment `BattleRevision`, and increment `TurnCount` only when the result is `Complete`. When a queued `Resume` returns `Complete`, set `PendingPhase(Some(BattlePhase::Idle))` so `commit_pending_phase` clears the choice phase after events drain. When a main `Action` returns `Complete`, keep the already-idle phase. On error: leave revision unchanged and show `rule_error_message`.

- [ ] **Step 5: Keep current animation and overlay behavior intact**

Retain `PendingPhase`, `PendingNobleCandidates`, `game_over_phase`, and `commit_pending_phase`. Ensure human discard and noble overlays still appear only after earlier events/animations drain.

- [ ] **Step 6: Run Battle unit tests and the full suite**

```powershell
cargo test battle::tests
cargo test
```

Expected: existing phase, input-gate, animation and action-mapping tests pass.

- [ ] **Step 7: Commit centralized submission**

```powershell
git add src/battle.rs
git commit -m "refactor(battle): centralize rule decision submission"
```

### Task 10: Run AI searches asynchronously from Battle

**Files:**
- Create: `src/battle/ai_runtime.rs`
- Modify: `src/battle.rs:1-65,270-420,480-495,1353-1410,1691-1730`
- Modify: `src/ai/mcts.rs`
- Modify: `src/ai/mod.rs`
- Test: `src/battle/ai_runtime.rs`

- [ ] **Step 1: Write request-token acceptance tests**

```rust
#[test]
fn only_exact_current_request_is_accepted() {
    let token = AiRequestToken {
        match_id: 10,
        state_version: 3,
        player: 1,
        context_kind: DecisionContextKind::MainTurn,
        request_id: 7,
    };
    assert!(token_matches(&token, 10, 3, 1, DecisionContextKind::MainTurn, 7));
    assert!(!token_matches(&token, 10, 4, 1, DecisionContextKind::MainTurn, 7));
    assert!(!token_matches(&token, 10, 3, 0, DecisionContextKind::MainTurn, 7));
    assert!(!token_matches(&token, 10, 3, 1, DecisionContextKind::Discard, 7));
}

#[test]
fn human_input_is_allowed_only_for_human_current_player() {
    let controllers = PlayerControllers::human_vs_cpu();
    assert!(controllers.is_human(0));
    assert!(!controllers.is_human(1));
}
```

- [ ] **Step 2: Verify runtime tests fail before the child module exists**

```powershell
cargo test battle::ai_runtime::tests
```

Expected: compilation fails because `ai_runtime` is absent.

- [ ] **Step 3: Define controllers, tokens and runtime resources**

Create `src/battle/ai_runtime.rs` and load it from `battle.rs` with `mod ai_runtime;`:

```rust
#[derive(Clone)]
pub(super) enum PlayerController {
    Human,
    Computer(MctsConfig),
}

#[derive(Resource, Clone)]
pub(super) struct PlayerControllers([PlayerController; 2]);

impl PlayerControllers {
    pub(super) fn human_vs_cpu() -> Self {
        Self([PlayerController::Human, PlayerController::Computer(MctsConfig::normal())])
    }

    pub(super) fn is_human(&self, player: PlayerId) -> bool {
        matches!(self.0[player], PlayerController::Human)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AiRequestToken {
    pub match_id: u64,
    pub state_version: u64,
    pub player: PlayerId,
    pub context_kind: DecisionContextKind,
    pub request_id: u64,
}

fn token_matches(
    token: &AiRequestToken,
    match_id: u64,
    state_version: u64,
    player: PlayerId,
    context_kind: DecisionContextKind,
    request_id: u64,
) -> bool {
    token.match_id == match_id
        && token.state_version == state_version
        && token.player == player
        && token.context_kind == context_kind
        && token.request_id == request_id
}

struct ActiveSearch {
    token: AiRequestToken,
    control: SearchControl,
    task: Task<AiTaskOutcome>,
}

#[derive(Resource)]
pub(super) struct AiRuntime {
    match_id: u64,
    next_request_id: u64,
    decision_index: u64,
    active: Option<ActiveSearch>,
    ready: Option<(AiRequestToken, AiSearchResult)>,
}
```

`AiTaskOutcome` must represent `Completed(AiSearchResult)`, `Fallback { decision: AiDecision, reason: AiTaskFailure }`, and `Cancelled` without allowing a background panic to reach Bevy. `AiTaskFailure` has `Search(AiError)` and `Panicked` variants.

- [ ] **Step 4: Start a search from a redacted observation**

`start_ai_search` derives context from `BattlePhase` or `PendingPhase`. Start only when the current controller is CPU, no active/ready request exists, and the game is not over. Build the seed from `match_id`, `decision_index`, and player using wrapping multiplication/xor. Pass only `AiObservation`, context, config, seed and `SearchControl` into the task:

```rust
let fallback_observation = observation.clone();
let fallback_context = context.clone();
let task = AsyncComputeTaskPool::get().spawn(async move {
    let searched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search(observation, context, player, seed, config, control_for_task)
    }));
    match searched {
        Ok(Ok(result)) => AiTaskOutcome::Completed(result),
        Ok(Err(AiError::Cancelled)) => AiTaskOutcome::Cancelled,
        Ok(Err(error)) => fallback_outcome(
            fallback_observation,
            fallback_context,
            seed,
            AiTaskFailure::Search(error),
        ),
        Err(_) => fallback_outcome(
            fallback_observation,
            fallback_context,
            seed,
            AiTaskFailure::Panicked,
        ),
    }
});
```

`fallback_outcome` determinizes the retained redacted observation with the retained seed and calls `fallback_decision`; it returns `Fallback` on success and `Cancelled` only when even a legal fallback cannot be constructed. The closure must not capture `BattleModel`, `GameState`, `Commands`, an ECS query or any event queue.

- [ ] **Step 5: Poll without blocking and hold early results until safe submission**

Use Bevy's re-exports, so no Cargo dependency is added:

```rust
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};

if active.task.is_finished() {
    if let Some(outcome) = block_on(poll_once(&mut active.task)) {
        runtime.accept_task_outcome(active.token, outcome);
    }
}
```

`submit_ready_ai_decision` pushes into `RuleDecisionQueue` only when token fields match current match/revision/player/context/request and `PendingEvents` is empty and `AnimationCounts::busy()` is false. Map `AiDecision::Action` to `QueuedRuleDecision::Action`, and discard/noble decisions to `QueuedRuleDecision::Resume`.

- [ ] **Step 6: Gate human input and suppress CPU overlays**

Extend `input_gate` to require `controllers.is_human(model.current_id())`. In `commit_pending_phase`, spawn discard/noble overlays only for a human controller. For CPU contexts, retain the phase without spawning an overlay; `start_ai_search` detects it and starts the next one-second decision.

Set status text to `AI THINKING…` while `active.is_some()`. A result that finishes during earlier animations remains in `ready` and is submitted after animations drain.

- [ ] **Step 7: Cancel safely on Battle cleanup and stale state**

Before replacing or removing an active search, call `active.control.cancel()` and drop the task handle. Increment `BattleRevision` after every successful rule mutation. If a ready result is stale, discard it and let `start_ai_search` create a fresh request. If a task fails or panics, log the error and queue the legal fallback included in the search request; cap retries for the same revision at one before recomputing from a new observation.

- [ ] **Step 8: Schedule runtime systems in deterministic order**

Use this relative order inside the existing chained Battle update tuple:

```text
human input systems
apply_rule_decisions
play_events / phase commit / animations
ai_runtime::cancel_stale_search
ai_runtime::start_ai_search
ai_runtime::poll_ai_search
ai_runtime::submit_ready_ai_decision
UI refresh systems
```

Starting is allowed while events/animations are active; submission is not.

- [ ] **Step 9: Run runtime, Battle and full tests**

```powershell
cargo test battle::ai_runtime::tests
cargo test battle::tests
cargo test
```

Expected: token mismatch tests pass, existing Battle tests remain green, and no direct `GameState` is captured by the spawned closure.

- [ ] **Step 10: Commit asynchronous integration**

```powershell
git add src/battle/ai_runtime.rs src/battle.rs src/ai/mcts.rs src/ai/mod.rs
git commit -m "feat(battle): run computer turns asynchronously"
```

### Task 11: Redact blind CPU reservations and label player roles

**Files:**
- Modify: `src/battle.rs:497-580,2227-2393`
- Test: `src/battle.rs`

- [ ] **Step 1: Write viewer-aware presentation tests**

```rust
#[test]
fn human_cannot_see_cpu_blind_reservation() {
    let blind = ReservedCard::new(5, ReserveOrigin::BlindDeck(CardLevel::Level1));
    assert!(!reservation_face_visible(0, 1, blind));
    assert!(reservation_face_visible(1, 1, blind));
}

#[test]
fn market_reservation_remains_public() {
    let public = ReservedCard::new(5, ReserveOrigin::Market);
    assert!(reservation_face_visible(0, 1, public));
}

#[test]
fn player_role_labels_are_stable() {
    assert_eq!(player_label(0), "YOU");
    assert_eq!(player_label(1), "CPU");
}
```

- [ ] **Step 2: Verify presentation helpers are absent**

```powershell
cargo test human_cannot_see_cpu_blind_reservation
cargo test market_reservation_remains_public
cargo test player_role_labels_are_stable
```

Expected: compilation fails for missing helpers.

- [ ] **Step 3: Implement pure visibility and label helpers**

```rust
const HUMAN_PLAYER: PlayerId = 0;

fn reservation_face_visible(viewer: PlayerId, owner: PlayerId, reserved: ReservedCard) -> bool {
    viewer == owner || reserved.is_public()
}

fn player_label(player: PlayerId) -> &'static str {
    if player == HUMAN_PLAYER { "YOU" } else { "CPU" }
}
```

Use `player_label` in full and compact player panels.

- [ ] **Step 4: Render a card back for hidden reservations**

In `refresh_battle_ui`, set `face_visible = reservation_face_visible(HUMAN_PLAYER, row.0, reserved)`. If true, call the existing mini-card function. If false, call a new `spawn_reserved_card_back` that renders the same 70×44 footprint, a muted border, and text `HIDDEN L{1|2|3}` derived from `ReserveOrigin::BlindDeck(level)`. Do not look up `reserved.card_id` on the hidden branch.

Only the human player's own reserved cards receive `Button`, `ReservedCardButton`, and `BattleAction::BuyReservedCard`. CPU rows never create input components.

- [ ] **Step 5: Run presentation and full tests**

```powershell
cargo test battle::tests
cargo test
```

Expected: visibility tests pass and all existing UI logic tests remain green.

- [ ] **Step 6: Commit fair hidden-card presentation**

```powershell
git add src/battle.rs
git commit -m "feat(battle): hide CPU blind reservations"
```

### Task 12: Add end-to-end legality, strength and release verification

**Files:**
- Modify: `src/ai/mcts.rs`
- Modify: `README.md`
- Test: `src/ai/mcts.rs`

- [ ] **Step 1: Add a normal-speed seeded AI game test**

Create a test helper that repeatedly builds the current player's observation, runs `search` with `MctsConfig::for_iterations(64)`, applies the returned decision to `SimulationState`, and continues pending contexts until the game ends:

```rust
#[test]
fn seeded_ai_games_terminate_with_only_legal_decisions() {
    for seed in 0..4 {
        let game = GameState::new_seeded(2, seed).unwrap();
        let mut simulation = SimulationState::new(game, DecisionContext::MainTurn);
        for decision_index in 0..600 {
            if simulation.game.is_over() {
                break;
            }
            let player = simulation.game.current_id();
            let observation = AiObservation::from_game(&simulation.game, player);
            let result = search(
                observation,
                simulation.context.clone(),
                player,
                seed ^ decision_index,
                MctsConfig::for_iterations(64),
                SearchControl::new(),
            )
            .unwrap();
            assert!(simulation.legal_decisions().unwrap().contains(&result.decision));
            simulation.apply_decision(result.decision).unwrap();
        }
        assert!(simulation.game.is_over(), "seed {seed} did not finish");
    }
}
```

- [ ] **Step 2: Run the seeded integration test**

```powershell
cargo test seeded_ai_games_terminate_with_only_legal_decisions -- --nocapture
```

Expected: four seeded games terminate within 600 decisions and every submitted decision was legal.

- [ ] **Step 3: Add an ignored strength benchmark against random play**

Use 100 seeds, play each seed twice with MCTS assigned to opposite seats, use 256 iterations per MCTS decision, and choose the random player's actions uniformly from `legal_decisions`. Count MCTS wins and assert:

```rust
#[test]
#[ignore = "manual AI strength benchmark"]
fn mcts_beats_random_at_least_sixty_five_percent() {
    let summary = benchmark_against_random(100, 256, 0xA11CE);
    assert!(
        summary.mcts_wins * 100 >= summary.games * 65,
        "MCTS wins {}/{} ({:.1}%)",
        summary.mcts_wins,
        summary.games,
        summary.win_rate() * 100.0,
    );
}
```

The harness must use fixed seeds, resolve discard/noble contexts for both policies, and count `200` total games.

Implement the harness with these concrete control functions:

```rust
struct BenchmarkSummary {
    games: u32,
    mcts_wins: u32,
}

impl BenchmarkSummary {
    fn win_rate(&self) -> f32 {
        self.mcts_wins as f32 / self.games as f32
    }
}

fn benchmark_against_random(seeds: u64, iterations: u32, base_seed: u64) -> BenchmarkSummary {
    let mut summary = BenchmarkSummary { games: 0, mcts_wins: 0 };
    for seed in 0..seeds {
        for mcts_seat in [0, 1] {
            let winner = play_benchmark_game(seed, mcts_seat, iterations, base_seed);
            summary.games += 1;
            summary.mcts_wins += u32::from(winner == mcts_seat);
        }
    }
    summary
}

fn play_benchmark_game(
    game_seed: u64,
    mcts_seat: PlayerId,
    iterations: u32,
    policy_seed: u64,
) -> PlayerId {
    let game = GameState::new_seeded(2, game_seed).unwrap();
    let mut simulation = SimulationState::new(game, DecisionContext::MainTurn);
    let mut random = StdRng::seed_from_u64(policy_seed ^ game_seed ^ mcts_seat as u64);
    for decision_index in 0..600_u64 {
        if let Some(winner) = simulation.game.winner {
            return winner;
        }
        let player = simulation.game.current_id();
        let decision = if player == mcts_seat {
            let observation = AiObservation::from_game(&simulation.game, player);
            search(
                observation,
                simulation.context.clone(),
                player,
                policy_seed ^ game_seed ^ decision_index,
                MctsConfig::for_iterations(iterations),
                SearchControl::new(),
            )
            .unwrap()
            .decision
        } else {
            let legal = simulation.legal_decisions().unwrap();
            legal[random.random_range(0..legal.len())].clone()
        };
        simulation.apply_decision(decision).unwrap();
    }
    panic!("benchmark game {game_seed} did not finish in 600 decisions");
}
```

- [ ] **Step 4: Run the ignored strength benchmark once**

```powershell
cargo test mcts_beats_random_at_least_sixty_five_percent -- --ignored --nocapture
```

Expected: `MCTS wins N/200` is printed and the win rate is at least 65%. If it fails, adjust only evaluation weights and rollout weighting, rerun the deterministic benchmark, and record the final values in `evaluation.rs`; do not weaken the threshold.

- [ ] **Step 5: Document the finished mode and diagnostics**

Add this concise section to `README.md`:

````markdown
## Computer opponent

New Adventure starts a two-player game with the human as Player 1 (`YOU`) and a computer as Player 2 (`CPU`). The computer uses a hidden-information MCTS search for up to one second per action, discard, or noble choice. Blind-reserved cards remain hidden from the opponent.

AI correctness tests run with `cargo test`. The longer deterministic strength check is available with:

```powershell
cargo test mcts_beats_random_at_least_sixty_five_percent -- --ignored --nocapture
```
````

- [ ] **Step 6: Run the complete release-quality command set**

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all three commands exit successfully with no warnings promoted to errors.

- [ ] **Step 7: Perform the manual responsive-UI check**

Run:

```powershell
cargo run
```

Verify all of these outcomes in one game:

1. Player labels show `YOU` and `CPU`, and `YOU` starts.
2. Human input locks immediately after a complete human turn.
3. Status shows `AI THINKING…` without freezing window movement or animations.
4. CPU action is submitted after earlier events and animations finish.
5. CPU automatically resolves discard and noble choices, each with its own search.
6. CPU blind reservations render as card backs; market-origin reservations remain visible.
7. Returning to the menu during CPU thinking does not crash or later submit a stale action.
8. A complete game reaches the existing Game Over overlay.

- [ ] **Step 8: Commit integration tests and documentation**

```powershell
git add src/ai/mcts.rs README.md
git commit -m "test(ai): verify legality and playing strength"
```

## Final review checkpoint

Before integration, compare every design requirement with the implementation:

```powershell
rg -n "time_limit|max_nodes|max_rollout_turns|rollout_epsilon" src/ai
rg -n "AiObservation::from_game|AsyncComputeTaskPool|AI THINKING|reservation_face_visible" src
git log --oneline -12
git status --short
```

Expected:

- Normal config is 1 second, 100,000 nodes, 60 complete rollout turns and epsilon 0.15.
- The spawned task receives `AiObservation`, not `GameState`.
- Human input, stale result rejection, cancellation and hidden reservation presentation are present.
- Each task above produced a focused commit.
- The isolated implementation worktree is clean after the planned commits; the original user workspace and its pre-existing changes remain untouched.
