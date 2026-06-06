use std::collections::VecDeque;

use tracing::debug;

use crate::session::{ConversationTurn, Session};

pub struct Context {
    pub system_prompt: String,
    pub turns: VecDeque<ConversationTurn>,
    pub user_message: String,
}

pub struct ContextManager {
    base_system_prompt: String,
    max_tokens: usize,
}

impl ContextManager {
    pub fn new(base_system_prompt: &String, max_tokens: usize) -> Self {
        let base_system_prompt = base_system_prompt.to_owned();

        Self {
            base_system_prompt,
            max_tokens,
        }
    }

    pub async fn build(
        &self,
        session: &Session,
        input: &str,
        _caller_conversation_id: Option<String>,
    ) -> Context {
        debug!(
            input_len = input.len(),
            session_turns = session.turns.len(),
            max_tokens = self.max_tokens,
            "building prompt context"
        );
        let mut turns = session.turns.clone();

        let max_turns = (self.max_tokens / 512).max(1);
        if turns.len() > max_turns {
            let drain_count = turns.len() - max_turns;
            for _ in 0 .. drain_count {
                turns.pop_front();
            }
            debug!(
                drained_turns = drain_count,
                "compacted conversation turns for context"
            );
        }

        let conversation_id = session.key.conversation_id.to_string();
        let system_prompt = self.build_system_prompt_with_memory(&conversation_id);

        Context {
            system_prompt,
            turns,
            user_message: input.to_string(),
        }
    }

    fn build_system_prompt_with_memory(&self, conversation_id: &str) -> String {
        let conversation_id = conversation_id.to_string();

        let base_system_prompt = self
            .base_system_prompt
            .replace("{conversation_id}", &conversation_id);

        let mut prompt = String::from("<nekoui_prompt>\n");
        prompt.push_str("  <system_instruction>");
        prompt.push_str(&escape_xml(&base_system_prompt));
        prompt.push_str("</system_instruction>\n");
        prompt.push_str("  <caller_context>\n");
        prompt.push_str(&format!(
            "    <conversation_id>{}</conversation_id>\n",
            conversation_id
        ));
        prompt.push_str("  </caller_context>\n");

        prompt.push_str("</nekoui_prompt>");
        prompt
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
