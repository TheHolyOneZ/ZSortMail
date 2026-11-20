use serde_json::Value;

use crate::client::GmailClient;
use crate::error::Result;

pub const PEOPLE_BASE: &str = "https://people.googleapis.com/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleContact {
    pub name: String,
    pub emails: Vec<String>,
    pub organization: Option<String>,
    pub title: Option<String>,
    pub note: Option<String>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn contact_from(person: &Value) -> Option<GoogleContact> {
    let emails: Vec<String> = person
        .get("emailAddresses")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|e| text(e, "value"))
        .map(|e| e.to_lowercase())
        .collect();
    if emails.is_empty() {
        return None;
    }
    let first = |list: &str| person.get(list).and_then(Value::as_array).and_then(|a| a.first());
    let name = first("names").and_then(|n| text(n, "displayName")).unwrap_or_default();
    let org = first("organizations");
    Some(GoogleContact {
        name,
        emails,
        organization: org.and_then(|o| text(o, "name")),
        title: org.and_then(|o| text(o, "title")),
        note: first("biographies").and_then(|b| text(b, "value")),
    })
}

pub async fn fetch_contacts(client: &GmailClient, base: &str) -> Result<Vec<GoogleContact>> {
    let url = format!("{}/people/me/connections", base.trim_end_matches('/'));
    let mut out = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..100 {
        let mut query = vec![
            ("personFields", "names,emailAddresses,organizations,biographies".to_string()),
            ("pageSize", "1000".to_string()),
        ];
        if let Some(t) = &token {
            query.push(("pageToken", t.clone()));
        }
        let page: Value = client.get_json(&url, &query).await?;
        if let Some(list) = page.get("connections").and_then(Value::as_array) {
            out.extend(list.iter().filter_map(contact_from));
        }
        token = text(&page, "nextPageToken");
        if token.is_none() {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_people_api_person_becomes_a_contact_and_one_without_email_is_dropped() {
        let person = json!({
            "names": [{"displayName": "Dr. Berg"}],
            "emailAddresses": [{"value": "Berg@Clinic.example"}, {"value": "berg2@clinic.example"}],
            "organizations": [{"name": "Clinic", "title": "Dentist"}],
            "biographies": [{"value": " Since 2019 "}]
        });
        let c = contact_from(&person).unwrap();
        assert_eq!(c.name, "Dr. Berg");
        assert_eq!(c.emails, vec!["berg@clinic.example", "berg2@clinic.example"]);
        assert_eq!(
            (c.organization.as_deref(), c.title.as_deref(), c.note.as_deref()),
            (Some("Clinic"), Some("Dentist"), Some("Since 2019"))
        );
        assert!(contact_from(&json!({"names": [{"displayName": "No Mail"}]})).is_none());
        assert!(contact_from(&json!({"emailAddresses": []})).is_none());
    }
}
