#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::collections::HashMap;

use crate::{GameEvent, Player, PlayerId};

#[derive(Debug, PartialEq)]
pub enum GameEventType {
    PlayerConnected,
    PlayerDisconnected,
    DiceRoll,
}

impl GameEvent {
    pub fn event_type(&self) -> GameEventType {
        match self {
            GameEvent::PlayerConnected { .. } => GameEventType::PlayerConnected,
            GameEvent::PlayerDisconnected { .. } => GameEventType::PlayerDisconnected,
            GameEvent::DiceRoll { .. } => GameEventType::DiceRoll,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum RuleResult {
    Complete,
    WaitingForEvent { event_type: GameEventType },
    GameOver,
}

pub trait Rule: Send + Sync {
    fn apply(&self, state: &mut GameState) -> RuleResult;
    fn validate(&self, state: &GameState, event: &GameEvent) -> bool;
    fn consume(&self, state: &mut GameState, event: &GameEvent) -> RuleResult;
}

#[derive(Default)]
pub struct GameState {
    /// The players in the game.
    pub players: HashMap<PlayerId, Player>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineStatus {
    Ready,
    GameOver,
}

pub struct GameEngine {
    game_state: GameState,
    current_rule_chain: Vec<Box<dyn Rule>>,
    current_rule_index: usize,
    waiting_for_event: Option<GameEventType>,
    engine_status: EngineStatus,
}

impl GameEngine {
    pub fn new(rule_chain: Vec<Box<dyn Rule>>) -> Self {
        Self {
            game_state: GameState::default(),
            current_rule_chain: rule_chain,
            current_rule_index: 0,
            waiting_for_event: None,
            engine_status: EngineStatus::Ready,
        }
    }

    pub fn game_state(&self) -> &GameState {
        &self.game_state
    }

    pub fn current_rule_index(&self) -> usize {
        self.current_rule_index
    }

    pub fn waiting_for_event(&self) -> &Option<GameEventType> {
        &self.waiting_for_event
    }

    pub fn engine_status(&self) -> EngineStatus {
        self.engine_status
    }

    // Process next rule
    pub fn process_next_rule(&mut self) {
        if self.current_rule_index >= self.current_rule_chain.len() {
            println!("[Engine] Turn complete!");
            return;
        }

        let rule = &self.current_rule_chain[self.current_rule_index];
        let result = rule.apply(&mut self.game_state);
        self.consume_rule_result(result);
    }

    pub fn validate(&self, event: &GameEvent) -> bool {
        if self.current_rule_index >= self.current_rule_chain.len() {
            return false;
        }

        let current_rule = &self.current_rule_chain[self.current_rule_index];
        current_rule.validate(&self.game_state, event)
    }

    pub fn consume(&mut self, event: &GameEvent) {
        match &self.waiting_for_event {
            Some(waiting) => {
                assert!(self.current_rule_index < self.current_rule_chain.len());
                debug_assert!(self.validate(event));

                println!("[Engine] Received event '{:?}', resuming...", event);
                let rule = &self.current_rule_chain[self.current_rule_index];
                let result = rule.consume(&mut self.game_state, event);
                self.consume_rule_result(result);
            }
            None => {
                println!("[Engine] Ingorning unexpected event: '{:?}'", event);
            }
        }
    }

    fn consume_rule_result(&mut self, result: RuleResult) {
        match result {
            RuleResult::Complete => {
                self.waiting_for_event = None;
                self.current_rule_index += 1;
                self.process_next_rule();
            }
            RuleResult::WaitingForEvent { event_type } => {
                self.waiting_for_event = Some(event_type);
            }
            RuleResult::GameOver {} => {
                println!("[Engine] Game over");
                self.waiting_for_event = None;
                self.engine_status = EngineStatus::GameOver;
            }
        };
    }
}
