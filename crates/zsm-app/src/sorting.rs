use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use zsm_core::dto::{GroupInfo, HiddenOverview, NewSender, PersonFlags, SenderDecision};
use zsm_core::store::actions::Touched;
use zsm_core::store::sorting::UndoOp;

use crate::core::Core;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BulkAction {
    Archive,
    Trash,
    Hide,
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Done {
    pub label: String,
    pub can_undo: bool,
}

pub(crate) struct UndoEntry {
    label: String,
    op: UndoOp,
}

const UNDO_DEPTH: usize = 50;

impl Core {
    pub(crate) fn remember(&self, label: impl Into<String>, op: UndoOp) -> Done {
        let label = label.into();
        let mut stack = self.lock(&self.undo);
        stack.push_back(UndoEntry { label: label.clone(), op });
        while stack.len() > UNDO_DEPTH {
            stack.pop_front();
        }
        Done { label, can_undo: true }
    }

    fn person_name(&self, id: i64) -> String {
        self.with_store(|s| Ok(s.person_summary(id)?.map(|p| p.name).unwrap_or_default())).unwrap_or_default()
    }

    fn after(&self, touched: &Touched) {
        self.touched(touched);
        self.changed("sorting", Vec::new());
    }

    pub fn mark_all_read(&self, person_id: Option<i64>) -> Result<Done> {
        let now = self.now_ms();
        let (count, ops, touched) = self.with_store(|s| {
            let ids = s.unread_conversation_ids(person_id)?;
            let mut ops = Vec::new();
            let mut touched = Touched::default();
            for id in &ids {
                let unread_ids = s.unread_message_ids(*id)?;
                let t = s.set_conversation_read(*id, true, now)?;
                touched.people.extend(t.people);
                touched.conversations.extend(t.conversations);
                ops.push(zsm_core::store::sorting::UndoOp::Read { conversation: *id, unread_ids });
            }
            touched.people.sort_unstable();
            touched.people.dedup();
            Ok((ids.len(), ops, touched))
        })?;
        if count == 0 {
            return Ok(Done { label: "Nothing was unread".into(), can_undo: false });
        }
        self.after(&touched);
        let what = match person_id {
            Some(id) => format!("Marked {} read", self.person_name(id)),
            None => "Marked everything read".to_string(),
        };
        Ok(self.remember(what, zsm_core::store::sorting::UndoOp::Many(ops)))
    }

    pub fn merge_people(&self, keep: i64, absorb: i64) -> Result<Done> {
        let (keep_name, absorb_name) = (self.person_name(keep), self.person_name(absorb));
        let (touched, undo) = self.with_store(|s| Ok(s.merge_people(keep, absorb)?))?;
        self.after(&touched);
        self.changed("people", vec![keep, absorb]);
        Ok(self.remember(format!("Merged {absorb_name} into {keep_name}"), undo))
    }

    pub fn split_person(&self, person: i64, address: &str) -> Result<Done> {
        let name = self.person_name(person);
        let now = self.now_ms();
        let (created, touched, undo) = self.with_store(|s| Ok(s.split_address(person, address, now)?))?;
        self.after(&touched);
        self.changed("people", vec![person, created]);
        Ok(self.remember(format!("Split {address} off {name}"), undo))
    }

    pub fn import_contacts(&self, text: &str) -> Result<zsm_core::store::contacts::ImportReport> {
        let contacts = zsm_core::vcard::parse_vcards(text);
        if contacts.is_empty() {
            return Err(crate::error::Error::Store(zsm_core::Error::Invalid(
                "No contacts with an email address were found in that file".into(),
            )));
        }
        let report = self.with_store(|s| Ok(s.import_contacts(&contacts)?))?;
        self.changed("people", Vec::new());
        Ok(report)
    }

    pub fn tags(&self) -> Result<Vec<zsm_core::store::tags::Tag>> {
        self.with_store(|s| Ok(s.list_tags()?))
    }

    pub fn conversation_tags(&self, id: i64) -> Result<Vec<zsm_core::store::tags::Tag>> {
        self.with_store(|s| Ok(s.conversation_tags(id)?))
    }

    pub fn tag_conversation(&self, id: i64, name: &str) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo) = self.with_store(|s| Ok(s.tag_conversation(id, name, now)?))?;
        self.after(&touched);
        self.changed("tags", vec![id]);
        Ok(self.remember(format!("Tagged {}", name.trim()), undo))
    }

    pub fn untag_conversation(&self, id: i64, label_id: i64) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo) = self.with_store(|s| Ok(s.untag_conversation(id, label_id, now)?))?;
        self.after(&touched);
        self.changed("tags", vec![id]);
        Ok(self.remember("Tag removed", undo))
    }

    pub fn bulk(&self, ids: &[i64], action: BulkAction) -> Result<Done> {
        let now = self.now_ms();
        let mut ops: Vec<UndoOp> = Vec::new();
        let mut touched = Touched::default();
        let mut changed = 0usize;
        self.with_store(|s| {
            for id in ids {
                let (t, op) = match action {
                    BulkAction::Archive => s.set_archived(*id, true, now)?,
                    BulkAction::Trash => s.set_trashed(*id, true, now)?,
                    BulkAction::Hide => s.set_conversation_hidden(*id, true, None, now)?,
                    BulkAction::Read => {
                        let unread_ids = s.unread_message_ids(*id)?;
                        if unread_ids.is_empty() {
                            continue;
                        }
                        let t = s.set_conversation_read(*id, true, now)?;
                        (t, UndoOp::Read { conversation: *id, unread_ids })
                    }
                };
                touched.people.extend(t.people);
                touched.conversations.extend(t.conversations);
                ops.push(op);
                changed += 1;
            }
            Ok(())
        })?;
        if changed == 0 {
            return Ok(Done { label: "Nothing to change".into(), can_undo: false });
        }
        touched.people.sort_unstable();
        touched.people.dedup();
        self.after(&touched);
        let noun = if changed == 1 { "conversation".to_string() } else { format!("{changed} conversations") };
        let label = match action {
            BulkAction::Archive => format!("Archived {noun}"),
            BulkAction::Trash => format!("Moved {noun} to trash"),
            BulkAction::Hide => format!("Hid {noun}"),
            BulkAction::Read => format!("Marked {noun} read"),
        };
        Ok(self.remember(label, UndoOp::Many(ops)))
    }

    pub fn archive_conversation(&self, id: i64, archived: bool) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo) = self.with_store(|s| Ok(s.set_archived(id, archived, now)?))?;
        self.after(&touched);
        Ok(self.remember(if archived { "Archived" } else { "Moved to inbox" }, undo))
    }

    pub fn trash_conversation(&self, id: i64, trashed: bool) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo) = self.with_store(|s| Ok(s.set_trashed(id, trashed, now)?))?;
        self.after(&touched);
        Ok(self.remember(if trashed { "Moved to trash" } else { "Restored from trash" }, undo))
    }

    pub fn hide_conversation(&self, id: i64, hidden: bool, until: Option<i64>) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo) = self.with_store(|s| Ok(s.set_conversation_hidden(id, hidden, until, now)?))?;
        self.after(&touched);
        Ok(self.remember(if hidden { "Conversation hidden" } else { "Conversation shown" }, undo))
    }

    pub fn hide_person(&self, id: i64, hidden: bool, until: Option<i64>) -> Result<Done> {
        let now = self.now_ms();
        let name = self.person_name(id);
        let (touched, undo) = self.with_store(|s| Ok(s.set_person_hidden(id, hidden, until, now)?))?;
        self.after(&touched);
        Ok(self.remember(if hidden { format!("{name} hidden") } else { format!("{name} is back") }, undo))
    }

    pub fn decide_sender(&self, id: i64, decision: SenderDecision) -> Result<Done> {
        let now = self.now_ms();
        let name = self.person_name(id);
        let (touched, undo) = self.with_store(|s| Ok(s.decide_sender(id, decision, now)?))?;
        self.after(&touched);
        let label = match decision {
            SenderDecision::Allow => format!("{name} allowed"),
            SenderDecision::Hide => format!("{name} hidden"),
            SenderDecision::Block => format!("{name} blocked"),
        };
        Ok(self.remember(label, undo))
    }

    pub fn decide_domain(&self, domain: &str, decision: SenderDecision) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo, _) = self.with_store(|s| Ok(s.decide_domain(domain, decision, now)?))?;
        self.after(&touched);
        let domain = domain.trim().trim_start_matches('@').to_lowercase();
        let label = match decision {
            SenderDecision::Allow => format!("Everyone at {domain} allowed"),
            SenderDecision::Hide => format!("Everyone at {domain} hidden"),
            SenderDecision::Block => format!("Everyone at {domain} blocked"),
        };
        Ok(self.remember(label, undo))
    }

    pub fn set_person_flags(&self, id: i64, flags: &PersonFlags) -> Result<Done> {
        let (touched, undo) = self.with_store(|s| Ok(s.set_person_flags(id, flags)?))?;
        self.after(&touched);
        Ok(self.remember("Saved", undo))
    }

    pub fn person_flags(&self, id: i64) -> Result<PersonFlags> {
        self.with_store(|s| Ok(s.person_flags(id)?))
    }

    pub fn undo(&self) -> Result<Option<String>> {
        let Some(entry) = self.lock(&self.undo).pop_back() else {
            return Ok(None);
        };
        let now = self.now_ms();
        let touched = self.with_store(|s| Ok(s.apply_undo(&entry.op, now)?))?;
        self.after(&touched);
        Ok(Some(entry.label))
    }

    pub fn undo_label(&self) -> Option<String> {
        self.lock(&self.undo).back().map(|e| e.label.clone())
    }

    pub fn new_senders(&self) -> Result<Vec<NewSender>> {
        self.with_store(|s| Ok(s.list_new_senders()?))
    }

    pub fn hidden(&self) -> Result<HiddenOverview> {
        self.with_store(|s| Ok(s.list_hidden()?))
    }

    pub fn set_domain_rule(&self, domain: &str, action: Option<&str>) -> Result<()> {
        let now = self.now_ms();
        self.with_store(|s| Ok(s.set_domain_rule(domain, action, now)?))?;
        self.changed("sorting", Vec::new());
        Ok(())
    }

    pub fn domain_rules(&self) -> Result<Vec<DomainRule>> {
        self.with_store(|s| {
            Ok(s.domain_rules()?.into_iter().map(|(domain, action)| DomainRule { domain, action }).collect())
        })
    }

    pub fn groups(&self) -> Result<Vec<GroupInfo>> {
        self.with_store(|s| Ok(s.list_groups()?))
    }

    pub fn people_in(
        &self,
        category: zsm_core::store::categories::CategoryRef,
        filter: zsm_core::dto::PeopleFilter,
        query: &str,
    ) -> Result<Vec<zsm_core::dto::PersonSummary>> {
        self.with_store(|s| Ok(s.list_people_in(category, filter, query)?))
    }

    pub fn save_category(&self, input: &zsm_core::store::categories::CategoryInput) -> Result<i64> {
        let now = self.now_ms();
        let id = self.with_store(|s| Ok(s.save_category(input, now)?))?;
        self.changed("groups", vec![id]);
        Ok(id)
    }

    pub fn category_members(&self, id: i64) -> Result<Vec<zsm_core::store::categories::CategoryMember>> {
        self.with_store(|s| Ok(s.category_members(id)?))
    }

    pub fn add_to_category(
        &self,
        group: i64,
        person: Option<i64>,
        email: Option<&str>,
        name: Option<&str>,
    ) -> Result<i64> {
        let now = self.now_ms();
        let id = self.with_store(|s| Ok(s.add_to_category(group, person, email, name, now)?))?;
        self.changed("groups", vec![group]);
        self.changed("people", vec![id]);
        Ok(id)
    }

    pub fn set_member_alias(&self, group: i64, person: i64, alias: &str) -> Result<()> {
        self.with_store(|s| Ok(s.set_member_alias(group, person, alias)?))?;
        self.changed("groups", vec![group]);
        Ok(())
    }

    pub fn remove_from_category(&self, group: i64, person: i64) -> Result<()> {
        self.with_store(|s| Ok(s.remove_from_category(group, person)?))?;
        self.changed("groups", vec![group]);
        Ok(())
    }

    pub fn order_categories(&self, ids: &[i64]) -> Result<()> {
        self.with_store(|s| Ok(s.order_categories(ids)?))?;
        self.changed("groups", Vec::new());
        Ok(())
    }

    pub fn mirror_tags(&self) -> Result<bool> {
        self.with_store(|s| Ok(s.setting("mirror_tags")?.as_deref() == Some("1")))
    }

    pub fn set_mirror_tags(&self, on: bool) -> Result<()> {
        self.with_store(|s| Ok(s.set_setting("mirror_tags", if on { "1" } else { "0" })?))
    }

    pub fn set_active_account(&self, account: Option<i64>) -> Result<()> {
        self.with_store(|s| {
            s.set_account_scope(account);
            Ok(())
        })?;
        self.changed("*", Vec::new());
        Ok(())
    }

    pub fn create_group(&self, name: &str, ink: u8) -> Result<i64> {
        let now = self.now_ms();
        let id = self.with_store(|s| Ok(s.create_group(name, ink, now)?))?;
        self.changed("groups", vec![id]);
        Ok(id)
    }

    pub fn delete_group(&self, id: i64) -> Result<()> {
        self.with_store(|s| Ok(s.delete_group(id)?))?;
        self.changed("groups", vec![id]);
        Ok(())
    }

    pub fn set_person_groups(&self, person_id: i64, group_ids: &[i64]) -> Result<()> {
        let touched = self.with_store(|s| Ok(s.set_person_groups(person_id, group_ids)?))?;
        self.touched(&touched);
        self.changed("groups", Vec::new());
        Ok(())
    }

    pub fn groups_of(&self, person_id: i64) -> Result<Vec<i64>> {
        self.with_store(|s| Ok(s.groups_of(person_id)?))
    }

    pub fn lift_expired_hides(&self) -> Result<()> {
        let now = self.now_ms();
        let touched = self.with_store(|s| Ok(s.lift_expired_hides(now)?))?;
        if !touched.people.is_empty() || !touched.conversations.is_empty() {
            self.after(&touched);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct DomainRule {
    pub domain: String,
    pub action: String,
}

pub(crate) type UndoStack = VecDeque<UndoEntry>;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use jiff::tz::TimeZone;
    use zsm_core::clock::FixedClock;
    use zsm_core::dto::PeopleFilter;

    use super::*;
    use crate::{Core, CoreConfig};

    fn core() -> Arc<Core> {
        let core = Core::new(CoreConfig {
            clock: Arc::new(FixedClock::new(1_795_082_400_000)),
            timezone: TimeZone::UTC,
            ..CoreConfig::in_memory()
        })
        .unwrap();
        core.start_demo(0).unwrap();
        core
    }

    #[test]
    fn each_change_says_what_it_did_and_undo_puts_back_the_last_one_first() {
        let core = core();
        let maya = core.people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let convos = core.conversations(maya).unwrap();
        let (a, b) = (convos[0].id, convos[1].id);
        assert_eq!(core.undo_label(), None);
        assert_eq!(core.undo().unwrap(), None, "nothing to undo yet");

        let done = core.archive_conversation(a, true).unwrap();
        assert_eq!(done, Done { label: "Archived".into(), can_undo: true });
        assert_eq!(core.trash_conversation(b, true).unwrap().label, "Moved to trash");
        assert_eq!(core.undo_label().as_deref(), Some("Moved to trash"));
        assert_eq!(core.conversations(maya).unwrap().len(), convos.len() - 1);

        assert_eq!(core.undo().unwrap().as_deref(), Some("Moved to trash"));
        assert_eq!(core.conversations(maya).unwrap().len(), convos.len());
        assert_eq!(core.undo().unwrap().as_deref(), Some("Archived"));
        assert_eq!(core.undo().unwrap(), None);
    }

    #[test]
    fn the_new_senders_deck_drives_people_and_the_rail_badge() {
        let core = core();
        assert_eq!(core.app_state().unwrap().counts.new_senders, 5);
        let waiting = core.new_senders().unwrap();
        let priya = waiting.iter().find(|n| n.person.name == "Priya Raman").unwrap().person.id;
        assert_eq!(core.decide_sender(priya, SenderDecision::Allow).unwrap().label, "Priya Raman allowed");
        assert_eq!(core.app_state().unwrap().counts.new_senders, 4);
        assert_eq!(core.people(PeopleFilter::All, "Priya").unwrap().len(), 1);
        core.undo().unwrap();
        assert_eq!(core.app_state().unwrap().counts.new_senders, 5);

        let marcus = waiting.iter().find(|n| n.person.name == "Marcus Lee").unwrap().person.id;
        assert_eq!(core.decide_sender(marcus, SenderDecision::Block).unwrap().label, "Marcus Lee blocked");
        assert!(core.hidden().unwrap().people.iter().any(|h| h.blocked && h.person.name == "Marcus Lee"));
        assert_eq!(core.hide_person(marcus, false, None).unwrap().label, "Marcus Lee is back");
        assert!(core.hidden().unwrap().people.is_empty());
    }

    #[test]
    fn the_undo_history_is_bounded() {
        let core = core();
        let maya = core.people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let convo = core.conversations(maya).unwrap()[0].id;
        for i in 0..(UNDO_DEPTH + 10) {
            core.archive_conversation(convo, i % 2 == 0).unwrap();
        }
        let mut undone = 0;
        while core.undo().unwrap().is_some() {
            undone += 1;
        }
        assert_eq!(undone, UNDO_DEPTH);
    }

    #[test]
    fn groups_and_domain_rules_round_trip() {
        let core = core();
        let work = core.create_group("Work", 3).unwrap();
        let maya = core.people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        core.set_person_groups(maya, &[work]).unwrap();
        assert_eq!(core.groups_of(maya).unwrap(), vec![work]);
        assert_eq!(core.groups().unwrap()[0].members, 1);
        core.delete_group(work).unwrap();
        assert!(core.groups().unwrap().is_empty());

        core.set_domain_rule("Shop.Example", Some("block")).unwrap();
        assert_eq!(
            core.domain_rules().unwrap(),
            vec![DomainRule { domain: "shop.example".into(), action: "block".into() }]
        );
        core.set_domain_rule("shop.example", None).unwrap();
        assert!(core.domain_rules().unwrap().is_empty());
    }
}
