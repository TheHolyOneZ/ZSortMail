use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::people::{SUMMARY_COLUMNS, parse_kind, refresh_person, summary_from_row};
use crate::dto::{PeopleFilter, PersonSummary};
use crate::error::{Error, Result};
use crate::ink::ink_for;
use crate::text::fts_prefix_query;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CategoryRef {
    Everyone,
    Favourites,
    Group { id: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CategoryInput {
    pub id: Option<i64>,
    pub name: String,
    pub ink: u8,
    pub icon: String,
    pub description: String,
    pub image: Option<String>,
    pub sidebar: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CategoryMember {
    pub person: PersonSummary,
    pub alias: Option<String>,
    pub email: String,
}

fn valid_image(image: &str) -> bool {
    let allowed = ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/webp;base64,"];
    image.len() <= 300_000
        && allowed.iter().any(|p| image.starts_with(p))
        && image
            .bytes()
            .skip(image.find(',').map_or(0, |i| i + 1))
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

const ICONS: &[&str] = &[
    "users",
    "heart",
    "star",
    "briefcase",
    "stethoscope",
    "house",
    "graduation-cap",
    "airplane",
    "shopping-bag",
    "bank",
    "wrench",
    "paw-print",
    "music-notes",
    "game-controller",
    "car",
    "tooth",
    "baby",
    "church",
    "football",
    "coffee",
    "book",
    "camera",
    "lifebuoy",
    "tree",
];

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("A category needs a name".into()));
    }
    if name.chars().count() > 40 {
        return Err(Error::Invalid("Category names can be up to 40 characters".into()));
    }
    if ["everyone", "favourites", "favorites"].contains(&name.to_lowercase().as_str()) {
        return Err(Error::Invalid(format!("“{name}” is already a built-in category")));
    }
    Ok(name.to_string())
}

impl Store {
    pub fn save_category(&mut self, input: &CategoryInput, now: i64) -> Result<i64> {
        let name = clean_name(&input.name)?;
        if !ICONS.contains(&input.icon.as_str()) {
            return Err(Error::Invalid("Pick an icon from the list".into()));
        }
        if input.description.chars().count() > 300 {
            return Err(Error::Invalid("The description can be up to 300 characters".into()));
        }
        if input.image.as_deref().is_some_and(|i| !valid_image(i)) {
            return Err(Error::Invalid("The picture must be a PNG, JPEG or WebP image under 200 KB".into()));
        }
        let taken: Option<i64> =
            self.conn().query_row("SELECT id FROM groups WHERE name = ?1", [&name], |r| r.get(0)).optional()?;
        if taken.is_some_and(|t| Some(t) != input.id) {
            return Err(Error::Invalid(format!("There is already a category called {name}")));
        }
        match input.id {
            Some(id) => {
                let changed = self.conn().execute(
                    "UPDATE groups SET name = ?2, ink = ?3, icon = ?4, description = ?5, image = ?6, sidebar = ?7 WHERE id = ?1",
                    params![id, name, input.ink.min(9), input.icon, input.description.trim(), input.image, input.sidebar],
                )?;
                if changed == 0 {
                    return Err(Error::NotFound("category"));
                }
                Ok(id)
            }
            None => {
                let position: i64 =
                    self.conn().query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM groups", [], |r| r.get(0))?;
                self.conn().execute(
                    "INSERT INTO groups (name, ink, icon, description, image, sidebar, position, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        name,
                        input.ink.min(9),
                        input.icon,
                        input.description.trim(),
                        input.image,
                        input.sidebar,
                        position,
                        now
                    ],
                )?;
                Ok(self.conn().last_insert_rowid())
            }
        }
    }

    pub fn list_people_in(
        &self,
        category: CategoryRef,
        filter: PeopleFilter,
        query: &str,
    ) -> Result<Vec<PersonSummary>> {
        let mut sql = format!(
            "SELECT {SUMMARY_COLUMNS} FROM people p WHERE p.status = 'known' AND p.kind <> 'self' AND p.hidden_at IS NULL"
        );
        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        match category {
            CategoryRef::Everyone => {
                sql.push_str(" AND p.last_message_at IS NOT NULL");
                sql.push_str(&self.in_account("p"));
            }
            CategoryRef::Favourites => {
                sql.push_str(" AND p.vip = 1 AND p.last_message_at IS NOT NULL");
                sql.push_str(&self.in_account("p"));
            }
            CategoryRef::Group { id } => {
                sql.push_str(" AND p.id IN (SELECT person_id FROM person_groups WHERE group_id = ?)");
                args.push(id.into());
            }
        }
        sql.push_str(match filter {
            PeopleFilter::All => "",
            PeopleFilter::NeedsReply => " AND p.needs_reply = 1",
            PeopleFilter::WaitingOnThem => " AND p.waiting_on_them = 1",
            PeopleFilter::HasFiles => " AND p.has_files = 1",
            PeopleFilter::Pinned => " AND p.pinned = 1",
        });
        if let Some(q) = fts_prefix_query(query) {
            sql.push_str(" AND p.id IN (SELECT rowid FROM people_fts WHERE people_fts MATCH ?)");
            args.push(q.into());
        }
        sql.push_str(
            " ORDER BY p.pinned DESC, COALESCE(p.last_message_at, 0) DESC, p.display_name COLLATE NOCASE, p.id",
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let mut people: Vec<PersonSummary> = stmt
            .query_map(rusqlite::params_from_iter(args), summary_from_row)?
            .collect::<std::result::Result<_, _>>()?;
        if let CategoryRef::Group { id } = category {
            let mut alias =
                self.conn().prepare("SELECT alias FROM person_groups WHERE group_id = ?1 AND person_id = ?2")?;
            for p in &mut people {
                if let Some(a) =
                    alias.query_row(params![id, p.id], |r| r.get::<_, Option<String>>(0)).optional()?.flatten()
                {
                    p.name = a;
                }
            }
        }
        Ok(people)
    }

    pub fn category_members(&self, group_id: i64) -> Result<Vec<CategoryMember>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {SUMMARY_COLUMNS}, pg.alias,
                    COALESCE((SELECT address FROM person_addresses WHERE person_id = p.id ORDER BY is_primary DESC LIMIT 1), '')
             FROM person_groups pg JOIN people p ON p.id = pg.person_id
             WHERE pg.group_id = ?1 ORDER BY p.display_name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map([group_id], |r| {
            Ok(CategoryMember { person: summary_from_row(r)?, alias: r.get(15)?, email: r.get(16)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_to_category(
        &mut self,
        group_id: i64,
        person_id: Option<i64>,
        email: Option<&str>,
        name: Option<&str>,
        now: i64,
    ) -> Result<i64> {
        self.in_transaction(|tx| {
            tx.query_row("SELECT 1 FROM groups WHERE id = ?1", [group_id], |_| Ok(())).optional()?.ok_or(Error::NotFound("category"))?;
            let name = name.map(str::trim).filter(|n| !n.is_empty());
            let person = match (person_id, email.map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty())) {
                (Some(id), _) => {
                    tx.query_row("SELECT 1 FROM people WHERE id = ?1 AND kind <> 'self'", [id], |_| Ok(())).optional()?.ok_or(Error::NotFound("person"))?;
                    id
                }
                (None, Some(email)) => {
                    if !super::drafts::valid_email(&email) {
                        return Err(Error::Invalid(format!("“{email}” is not an email address")));
                    }
                    let known: Option<i64> =
                        tx.query_row("SELECT person_id FROM person_addresses WHERE address = ?1", [&email], |r| r.get(0)).optional()?;
                    match known {
                        Some(id) => id,
                        None => {
                            let account: i64 = tx
                                .query_row("SELECT id FROM accounts ORDER BY id LIMIT 1", [], |r| r.get(0))
                                .optional()?
                                .ok_or_else(|| Error::Invalid("Link a mailbox first".into()))?;
                            let shown = name.map(str::to_string).unwrap_or_else(|| super::ingest::prettify_local_part(&email));
                            tx.execute(
                                "INSERT INTO people (account_id, kind, status, display_name, ink, created_at) VALUES (?1, 'person', 'known', ?2, ?3, ?4)",
                                params![account, shown, ink_for(&email), now],
                            )?;
                            let id = tx.last_insert_rowid();
                            tx.execute("INSERT INTO person_addresses (account_id, address, person_id, is_primary) VALUES (?1, ?2, ?3, 1)", params![account, email, id])?;
                            refresh_person(tx, id)?;
                            id
                        }
                    }
                }
                (None, None) => return Err(Error::Invalid("Pick a person or type an email address".into())),
            };
            tx.execute(
                "INSERT INTO person_groups (person_id, group_id, alias) VALUES (?1, ?2, ?3)
                 ON CONFLICT (person_id, group_id) DO UPDATE SET alias = COALESCE(?3, alias)",
                params![person, group_id, name],
            )?;
            let _ = parse_kind;
            Ok(person)
        })
    }

    pub fn set_member_alias(&mut self, group_id: i64, person_id: i64, alias: &str) -> Result<()> {
        let alias = alias.trim();
        if alias.chars().count() > 60 {
            return Err(Error::Invalid("Names can be up to 60 characters".into()));
        }
        let changed = self.conn().execute(
            "UPDATE person_groups SET alias = ?3 WHERE group_id = ?1 AND person_id = ?2",
            params![group_id, person_id, (!alias.is_empty()).then_some(alias)],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("member"));
        }
        Ok(())
    }

    pub fn remove_from_category(&mut self, group_id: i64, person_id: i64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM person_groups WHERE group_id = ?1 AND person_id = ?2",
            params![group_id, person_id],
        )?;
        Ok(())
    }

    pub fn order_categories(&mut self, ids: &[i64]) -> Result<()> {
        self.in_transaction(|tx| {
            for (position, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE groups SET position = ?2 WHERE id = ?1",
                    params![id, i64::try_from(position).unwrap_or(0)],
                )?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::rules::tests::{NOW, demo};

    fn input(name: &str) -> CategoryInput {
        CategoryInput {
            id: None,
            name: name.into(),
            ink: 4,
            icon: "stethoscope".into(),
            description: "People who look after me".into(),
            image: None,
            sidebar: false,
        }
    }

    fn person(store: &Store, q: &str) -> i64 {
        store.list_people(PeopleFilter::All, q).unwrap()[0].id
    }

    #[test]
    fn a_category_is_made_filled_and_shows_only_its_people_under_their_own_names() {
        let mut store = demo();
        let doctors = store.save_category(&input("Doctors"), NOW).unwrap();
        let hanna = person(&store, "Hanna");
        store.add_to_category(doctors, Some(hanna), None, Some("Dr. Berg"), NOW).unwrap();

        let all = store.list_people_in(CategoryRef::Everyone, PeopleFilter::All, "").unwrap();
        let inside = store.list_people_in(CategoryRef::Group { id: doctors }, PeopleFilter::All, "").unwrap();
        assert!(all.len() > 10 && inside.len() == 1, "Everyone has them all, the category only its own");
        assert_eq!(inside[0].name, "Dr. Berg", "the category's name for them");
        assert_eq!(store.get_person(hanna).unwrap().unwrap().summary.name, "Hanna Berg", "their own name is untouched");

        assert_eq!(
            store.list_people_in(CategoryRef::Group { id: doctors }, PeopleFilter::All, "dentist").unwrap().len(),
            1
        );
        assert!(
            store.list_people_in(CategoryRef::Group { id: doctors }, PeopleFilter::All, "okafor").unwrap().is_empty()
        );

        store.set_member_alias(doctors, hanna, "Hanna (dentist)").unwrap();
        assert_eq!(store.category_members(doctors).unwrap()[0].alias.as_deref(), Some("Hanna (dentist)"));
        store.set_member_alias(doctors, hanna, "  ").unwrap();
        assert_eq!(
            store.list_people_in(CategoryRef::Group { id: doctors }, PeopleFilter::All, "").unwrap()[0].name,
            "Hanna Berg"
        );
        assert!(store.set_member_alias(doctors, 999_999, "x").is_err());
    }

    #[test]
    fn a_person_can_be_in_several_categories_and_an_address_with_no_mail_yet_is_kept_ready() {
        let mut store = demo();
        let friends = store.save_category(&CategoryInput { icon: "heart".into(), ..input("Friends") }, NOW).unwrap();
        let family = store.save_category(&CategoryInput { icon: "house".into(), ..input("Family") }, NOW).unwrap();
        let dad = person(&store, "Dad");
        store.add_to_category(friends, Some(dad), None, None, NOW).unwrap();
        store.add_to_category(family, Some(dad), None, None, NOW).unwrap();
        assert_eq!(store.groups_of(dad).unwrap().len(), 2);
        assert_eq!(store.list_groups().unwrap().iter().map(|g| g.members).collect::<Vec<_>>(), vec![1, 1]);

        let before = store.list_people(PeopleFilter::All, "").unwrap().len();
        let doc = store.add_to_category(family, None, Some("Dr.Aydin@Clinic.example"), Some("Dr. Aydin"), NOW).unwrap();
        assert_eq!(
            store.list_people(PeopleFilter::All, "").unwrap().len(),
            before,
            "not in Everyone until there is mail"
        );
        let members = store.category_members(family).unwrap();
        assert!(members.iter().any(|m| m.person.id == doc && m.email == "dr.aydin@clinic.example"));
        assert_eq!(store.list_people_in(CategoryRef::Group { id: family }, PeopleFilter::All, "").unwrap().len(), 2);
        assert_eq!(store.add_to_category(family, None, Some("dr.aydin@clinic.example"), None, NOW).unwrap(), doc);
        let mut msg =
            crate::store::rules::tests::incoming("aydin1", "dr.aydin@clinic.example", "Your results", "All clear.");
        msg.from = crate::store::ingest::Address::new("dr.aydin@clinic.example", Some("Aydin"));
        crate::store::rules::tests::arrive(&mut store, &msg, true);
        assert_eq!(store.list_people(PeopleFilter::All, "").unwrap().len(), before + 1);
        assert_eq!(store.list_people(PeopleFilter::All, "results").unwrap()[0].id, doc);

        assert!(store.add_to_category(family, None, Some("not an address"), None, NOW).is_err());
        assert!(store.add_to_category(family, None, None, None, NOW).is_err());
        assert!(store.add_to_category(999, Some(dad), None, None, NOW).is_err());
    }

    #[test]
    fn favourites_are_a_category_of_their_own_and_the_rules_for_names_and_pictures_hold() {
        let mut store = demo();
        let maya = person(&store, "Okafor");
        store.set_person_flags(maya, &crate::dto::PersonFlags { whitelisted: false, vip: true, muted: false }).unwrap();
        let favourites = store.list_people_in(CategoryRef::Favourites, PeopleFilter::All, "").unwrap();
        assert_eq!(favourites.iter().map(|p| p.id).collect::<Vec<_>>(), vec![maya]);
        assert!(favourites[0].vip);
        assert!(
            !store
                .list_people_in(CategoryRef::Everyone, PeopleFilter::All, "")
                .unwrap()
                .iter()
                .filter(|p| p.id != maya)
                .any(|p| p.vip)
        );

        for bad in ["", "   ", "Everyone", "favourites", &"x".repeat(41)] {
            assert!(store.save_category(&input(bad), NOW).is_err(), "{bad}");
        }
        assert!(
            store.save_category(&CategoryInput { icon: "rocket-ship".into(), ..input("A") }, NOW).is_err(),
            "icons come from the list"
        );
        let first = store.save_category(&input("Work"), NOW).unwrap();
        assert!(store.save_category(&input("work"), NOW).is_err(), "names are unique, ignoring case");
        store.save_category(&CategoryInput { id: Some(first), ..input("Work") }, NOW).unwrap();

        let png = format!("data:image/png;base64,{}", "iVBORw0KGgo=");
        store
            .save_category(&CategoryInput { id: Some(first), image: Some(png.clone()), ..input("Work") }, NOW)
            .unwrap();
        assert_eq!(store.list_groups().unwrap()[0].image.as_deref(), Some(png.as_str()));
        for bad in [
            "data:text/html;base64,PHNjcmlwdD4=",
            "javascript:alert(1)",
            "data:image/png;base64,<script>",
            "https://x.example/a.png",
        ] {
            assert!(
                store
                    .save_category(&CategoryInput { id: Some(first), image: Some(bad.into()), ..input("Work") }, NOW)
                    .is_err(),
                "{bad}"
            );
        }
        assert!(
            store
                .save_category(
                    &CategoryInput {
                        id: Some(first),
                        image: Some(format!("data:image/png;base64,{}", "A".repeat(300_001))),
                        ..input("Work")
                    },
                    NOW
                )
                .is_err()
        );

        let second = store.save_category(&input("Hobbies"), NOW).unwrap();
        store.order_categories(&[second, first]).unwrap();
        assert_eq!(store.list_groups().unwrap().iter().map(|g| g.id).collect::<Vec<_>>(), vec![second, first]);
    }

    #[test]
    fn a_profile_shows_only_its_own_accounts_mail() {
        let mut store = demo();
        let everyone = store.list_people(PeopleFilter::All, "").unwrap().len();
        let first = store.accounts().unwrap()[0].id;
        let second = store.upsert_account("gmail", "work@example.test", None, NOW).unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO people (account_id, kind, status, display_name, ink, created_at, last_message_at, last_subject)
                 VALUES (?1, 'person', 'known', 'Wanda Work', 2, ?2, ?2, 'Budget')",
                params![second, NOW],
            )
            .unwrap();

        assert_eq!(store.list_people(PeopleFilter::All, "").unwrap().len(), everyone + 1, "no profile: all accounts");
        store.set_account_scope(Some(second));
        let only = store.list_people(PeopleFilter::All, "").unwrap();
        assert_eq!(only.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Wanda Work"]);
        assert_eq!(store.list_people_in(CategoryRef::Everyone, PeopleFilter::All, "").unwrap().len(), 1);
        assert!(store.search("Hanna", NOW, 10).unwrap().people.is_empty(), "search stays inside the profile");
        assert!(store.inbox(None, NOW, 50, 0).unwrap().is_empty());
        assert_eq!(store.counts().unwrap().1, 0);

        store.set_account_scope(Some(first));
        assert_eq!(store.list_people(PeopleFilter::All, "").unwrap().len(), everyone);
        assert!(!store.inbox(None, NOW, 50, 0).unwrap().is_empty());
        store.set_account_scope(None);
        assert_eq!(store.list_people(PeopleFilter::All, "").unwrap().len(), everyone + 1);
    }
}
