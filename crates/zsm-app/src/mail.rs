use zsm_core::dto::{
    ConversationDetail, ConversationSummary, FileEntry, MessageView, PeopleFilter, PersonDetail, PersonSummary,
    SearchResults,
};
use zsm_core::store::actions::PersonEdit;

use crate::core::Core;
use crate::error::{Error, Result};

impl Core {
    pub fn people(&self, filter: PeopleFilter, query: &str) -> Result<Vec<PersonSummary>> {
        self.with_store(|s| Ok(s.list_people(filter, query)?))
    }

    pub fn person(&self, id: i64) -> Result<Option<PersonDetail>> {
        self.with_store(|s| Ok(s.get_person(id)?))
    }

    pub fn conversations(&self, person_id: i64) -> Result<Vec<ConversationSummary>> {
        self.with_store(|s| Ok(s.list_conversations(person_id)?))
    }

    pub fn conversation(&self, id: i64, viewer: Option<i64>) -> Result<Option<ConversationDetail>> {
        self.with_store(|s| Ok(s.get_conversation(id, viewer)?))
    }

    pub fn files(&self, person_id: i64) -> Result<Vec<FileEntry>> {
        self.with_store(|s| Ok(s.list_files(person_id)?))
    }

    pub fn search(&self, query: &str) -> Result<SearchResults> {
        let now = self.now_ms();
        self.with_store(|s| Ok(s.search(query, now, 40)?))
    }

    pub fn set_conversation_read(&self, conversation_id: i64, read: bool) -> Result<()> {
        let now = self.now_ms();
        let touched = self.with_store(|s| Ok(s.set_conversation_read(conversation_id, read, now)?))?;
        self.touched(&touched);
        Ok(())
    }

    pub fn set_message_starred(&self, message_id: i64, starred: bool) -> Result<()> {
        let now = self.now_ms();
        let touched = self.with_store(|s| Ok(s.set_message_starred(message_id, starred, now)?))?;
        self.touched(&touched);
        Ok(())
    }

    pub fn set_person_pinned(&self, person_id: i64, pinned: bool) -> Result<()> {
        let touched = self.with_store(|s| Ok(s.set_person_pinned(person_id, pinned)?))?;
        self.touched(&touched);
        Ok(())
    }

    pub fn set_person_notes(&self, person_id: i64, notes: &str) -> Result<()> {
        let touched = self.with_store(|s| Ok(s.set_person_notes(person_id, notes)?))?;
        self.touched(&touched);
        Ok(())
    }

    pub fn edit_person(&self, person_id: i64, edit: &PersonEdit) -> Result<()> {
        let touched = self.with_store(|s| Ok(s.edit_person(person_id, edit)?))?;
        self.touched(&touched);
        Ok(())
    }

    pub fn timeline(
        &self,
        person_id: i64,
        before: Option<i64>,
    ) -> Result<Vec<zsm_core::store::timeline::TimelineEntry>> {
        self.with_store(|s| Ok(s.person_timeline(person_id, before, 60)?))
    }

    pub fn reply(&self, conversation_id: i64, text: &str) -> Result<MessageView> {
        let now = self.now_ms();
        let outcome = self.with_store(|s| Ok(s.store_reply(conversation_id, text, now)?))?;
        self.changed("conversations", vec![conversation_id]);
        self.changed("people", outcome.person_ids.clone());
        self.changed("messages", vec![outcome.message_id]);
        self.wake_sync();

        let detail =
            self.conversation(conversation_id, None)?.ok_or(Error::Store(zsm_core::Error::NotFound("conversation")))?;
        detail
            .messages
            .into_iter()
            .find(|m| m.id == outcome.message_id)
            .ok_or(Error::Store(zsm_core::Error::NotFound("message")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use jiff::tz::TimeZone;
    use zsm_core::clock::FixedClock;

    use super::*;
    use crate::events::{CoreEvent, EventSink};
    use crate::{Core, CoreConfig};

    const NOW: i64 = 1_795_082_400_000;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<CoreEvent>>);

    impl EventSink for Recorder {
        fn emit(&self, event: CoreEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    fn core() -> (Arc<Core>, Arc<Recorder>) {
        let recorder = Arc::new(Recorder::default());
        let core = Core::new(CoreConfig {
            clock: Arc::new(FixedClock::new(NOW)),
            sink: recorder.clone(),
            timezone: TimeZone::UTC,
            ..CoreConfig::in_memory()
        })
        .unwrap();
        core.start_demo(0).unwrap();
        recorder.0.lock().unwrap().clear();
        (core, recorder)
    }

    fn maya(core: &Core) -> i64 {
        core.people(PeopleFilter::All, "Okafor").unwrap()[0].id
    }

    #[test]
    fn starting_the_demo_twice_does_not_duplicate_it() {
        let (core, _) = core();
        let before = core.people(PeopleFilter::All, "").unwrap().len();
        core.start_demo(0).unwrap();
        assert_eq!(core.people(PeopleFilter::All, "").unwrap().len(), before);
        assert_eq!(core.app_state().unwrap().accounts.len(), 1);
    }

    #[test]
    fn app_state_counts_what_the_rail_badges_show() {
        let (core, _) = core();
        let state = core.app_state().unwrap();
        assert_eq!(state.counts.new_senders, 5);
        assert!(state.counts.unread >= 5, "unread: {}", state.counts.unread);
        assert!(state.schema_version >= 5, "every migration applied: {}", state.schema_version);
    }

    #[test]
    fn replying_stores_the_message_threads_it_and_tells_the_ui() {
        let (core, events) = core();
        let maya = maya(&core);
        let convo = core.conversations(maya).unwrap().into_iter().find(|c| c.subject == "Coffee Thursday?").unwrap();
        let sent = core.reply(convo.id, "  See you at 9:30.  ").unwrap();
        assert_eq!(sent.text, "See you at 9:30.");
        assert!(sent.from.is_me);
        assert_eq!(sent.to_line, "to Maya");

        let detail = core.conversation(convo.id, Some(maya)).unwrap().unwrap();
        assert_eq!(detail.messages.len(), convo.message_count as usize + 1);
        assert_eq!(detail.messages.last().unwrap().id, sent.id);
        assert_eq!(core.people(PeopleFilter::All, "").unwrap().iter().find(|p| p.id == maya).unwrap().last_at, NOW);

        let seen = events.0.lock().unwrap().clone();
        assert!(seen.contains(&CoreEvent::StoreChanged { entity: "conversations".into(), ids: vec![convo.id] }));
        assert!(seen.iter().any(|e| matches!(e, CoreEvent::StoreChanged { entity, .. } if entity == "people")));
    }

    #[test]
    fn replying_to_my_own_last_message_writes_to_the_same_people_and_re_is_not_stacked() {
        let (core, _) = core();
        let maya = maya(&core);
        let convo = core.conversations(maya).unwrap().into_iter().find(|c| c.subject == "Coffee Thursday?").unwrap();
        let first = core.reply(convo.id, "One more thing.").unwrap();
        let second = core.reply(convo.id, "And another.").unwrap();
        assert_eq!(second.to_line, first.to_line);
    }

    #[test]
    fn an_empty_reply_is_refused_with_a_message_to_show() {
        let (core, _) = core();
        let maya = maya(&core);
        let convo = core.conversations(maya).unwrap().remove(0);
        let err = core.reply(convo.id, "   \n ").unwrap_err();
        assert_eq!(err.to_string(), "Write something first");
        assert!(core.reply(999_999, "hello").is_err());
    }

    #[test]
    fn a_burst_of_changes_is_merged_into_one_event_per_entity() {
        let recorder = Arc::new(Recorder::default());
        let core = Core::new(CoreConfig {
            sink: recorder.clone(),
            coalesce: std::time::Duration::from_millis(30),
            ..CoreConfig::in_memory()
        })
        .unwrap();
        for id in [1, 2, 3, 2] {
            core.changed("people", vec![id]);
        }
        core.changed("conversations", vec![9]);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let seen = recorder.0.lock().unwrap().clone();
        assert_eq!(
            seen,
            [
                CoreEvent::StoreChanged { entity: "conversations".into(), ids: vec![9] },
                CoreEvent::StoreChanged { entity: "people".into(), ids: vec![1, 2, 3] },
            ]
        );
    }
}
