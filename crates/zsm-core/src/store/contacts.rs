use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::people::refresh_person;
use crate::error::Result;
use crate::vcard::Contact;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ImportReport {
    pub contacts: u32,
    pub matched: u32,
    pub updated: u32,
    pub unmatched: u32,
}

impl Store {
    pub fn import_contacts(&mut self, contacts: &[Contact]) -> Result<ImportReport> {
        self.in_transaction(|tx| {
            let mut report = ImportReport { contacts: u32::try_from(contacts.len()).unwrap_or(u32::MAX), ..Default::default() };
            for c in contacts {
                let mut person: Option<i64> = None;
                for email in &c.emails {
                    person = tx
                        .query_row("SELECT person_id FROM person_addresses WHERE address = ?1", [email], |r| r.get(0))
                        .optional()?;
                    if person.is_some() {
                        break;
                    }
                }
                let Some(id) = person else {
                    report.unmatched += 1;
                    continue;
                };
                report.matched += 1;
                let (name, title, organization, description, primary): (String, Option<String>, Option<String>, Option<String>, String) = tx.query_row(
                    "SELECT p.display_name, p.title, p.organization, p.description,
                            COALESCE((SELECT address FROM person_addresses WHERE person_id = p.id ORDER BY is_primary DESC LIMIT 1), '')
                     FROM people p WHERE p.id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )?;
                let blank = |v: &Option<String>| v.as_deref().is_none_or(|s| s.trim().is_empty());
                let unnamed = name.trim().is_empty() || name.eq_ignore_ascii_case(&primary);
                let new_name = (unnamed && !c.name.trim().is_empty()).then(|| c.name.clone());
                let new_title = if blank(&title) { c.title.clone() } else { None };
                let new_org = if blank(&organization) { c.organization.clone() } else { None };
                let new_desc = if blank(&description) { c.note.clone() } else { None };
                if new_name.is_none() && new_title.is_none() && new_org.is_none() && new_desc.is_none() {
                    continue;
                }
                tx.execute(
                    "UPDATE people SET display_name = COALESCE(?2, display_name), title = COALESCE(?3, title),
                       organization = COALESCE(?4, organization), description = COALESCE(?5, description) WHERE id = ?1",
                    params![id, new_name, new_title, new_org, new_desc],
                )?;
                refresh_person(tx, id)?;
                report.updated += 1;
            }
            Ok(report)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::dto::PeopleFilter;
    use crate::store::rules::tests::demo;
    use crate::vcard::parse_vcards;

    #[test]
    fn an_address_book_fills_blanks_and_never_overwrites_or_creates() {
        let mut store = demo();
        let maya = store.list_people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        store
            .edit_person(
                maya,
                &crate::store::actions::PersonEdit {
                    name: "Maya Okafor".into(),
                    title: Some("Design lead".into()),
                    organization: None,
                    description: None,
                    ink: 0,
                },
            )
            .unwrap();
        let before = store.list_people(PeopleFilter::All, "").unwrap().len();

        let cards = parse_vcards(
            "BEGIN:VCARD\nFN:Maya O.\nEMAIL:maya.okafor@northwind.example\nORG:Northwind\nTITLE:CEO\nNOTE:Met at the offsite\nEND:VCARD\n\
             BEGIN:VCARD\nFN:Nobody Known\nEMAIL:nobody@nowhere.example\nEND:VCARD\n",
        );
        let report = store.import_contacts(&cards).unwrap();
        assert_eq!((report.contacts, report.matched, report.updated, report.unmatched), (2, 1, 1, 1));
        let detail = store.get_person(maya).unwrap().unwrap();
        assert_eq!(detail.summary.title.as_deref(), Some("Design lead"), "a role you set is kept");
        assert_eq!(detail.summary.name, "Maya Okafor", "a name you set is kept");
        assert_eq!(detail.organization.as_deref(), Some("Northwind"), "a blank is filled");
        assert_eq!(detail.description.as_deref(), Some("Met at the offsite"));
        assert_eq!(store.list_people(PeopleFilter::All, "").unwrap().len(), before, "nobody was created");
        assert_eq!(store.import_contacts(&cards).unwrap().updated, 0);
    }
}
