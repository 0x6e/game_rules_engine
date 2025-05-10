#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::collections::HashMap;

use crate::{GameEvent, Player, PlayerId};

#[derive(Debug, PartialEq)]
pub enum RuleResult {
    Complete,
    WaitingForEvent,
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
    WaitingForEvent,
    GameOver,
}

pub struct GameEngine {
    game_state: GameState,
    current_rule_chain: Vec<Box<dyn Rule>>,
    current_rule_index: usize,
    engine_status: EngineStatus,
}

impl GameEngine {
    pub fn new(rule_chain: Vec<Box<dyn Rule>>) -> Self {
        Self {
            game_state: GameState::default(),
            current_rule_chain: rule_chain,
            current_rule_index: 0,
            engine_status: EngineStatus::Ready,
        }
    }

    pub fn game_state(&self) -> &GameState {
        &self.game_state
    }

    pub fn current_rule_index(&self) -> usize {
        self.current_rule_index
    }

    pub fn is_waiting_for_event(&self) -> bool {
        self.engine_status == EngineStatus::WaitingForEvent
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
        if !self.is_waiting_for_event() {
            println!("[Engine] Ingorning unexpected event: '{:?}'", event);
            return;
        }

        assert!(self.current_rule_index < self.current_rule_chain.len());

        println!("[Engine] Received event '{:?}', resuming...", event);
        debug_assert!(self.validate(event));
        let rule = &self.current_rule_chain[self.current_rule_index];
        let result = rule.consume(&mut self.game_state, event);
        self.consume_rule_result(result);
    }

    fn consume_rule_result(&mut self, result: RuleResult) {
        match result {
            RuleResult::Complete => {
                self.engine_status = EngineStatus::Ready;
                self.current_rule_index += 1;
                self.process_next_rule();
            }
            RuleResult::WaitingForEvent => {
                self.engine_status = EngineStatus::WaitingForEvent;
            }
            RuleResult::GameOver {} => {
                println!("[Engine] Game over");
                self.engine_status = EngineStatus::GameOver;
            }
        };
    }
}
