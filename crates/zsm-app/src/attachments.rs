use zsm_gmail::Error as GmailError;

use crate::core::Core;
use crate::error::{Error, Result};

impl Core {
    pub async fn prepare_attachment(&self, id: i64) -> Result<()> {
        let record =
            self.with_store(|s| Ok(s.attachment(id)?))?.ok_or(Error::Store(zsm_core::Error::NotFound("file")))?;
        if self.attachment_bytes(&record).is_some() {
            return Ok(());
        }
        let origin = self
            .with_store(|s| Ok(s.attachment_origin(id)?))?
            .ok_or(Error::Store(zsm_core::Error::NotFound("file")))?;
        let (message_id, attachment_id) = (origin.message_provider_id.as_str(), origin.attachment_provider_id.as_str());
        let client = self.client_for(origin.account_id).ok_or_else(|| {
            Error::Store(zsm_core::Error::Invalid("Link this account again to download its files.".into()))
        })?;
        let dir = self
            .data_dir
            .as_ref()
            .ok_or_else(|| Error::Store(zsm_core::Error::Invalid("There is nowhere to keep downloaded files".into())))?
            .join("attachments");
        std::fs::create_dir_all(&dir)?;

        let bytes = match client.attachment(message_id, attachment_id).await {
            Ok(bytes) => bytes,
            Err(GmailError::Api { status: 404, .. }) => {
                let fresh = client.get_message(message_id, zsm_gmail::client::Format::Full).await?;
                let incoming = zsm_gmail::convert::to_incoming(&fresh)?;
                let found = incoming
                    .attachments
                    .iter()
                    .find(|a| a.filename == origin.filename && a.size == origin.size)
                    .and_then(|a| a.provider_id.clone())
                    .ok_or(Error::Store(zsm_core::Error::NotFound("file")))?;
                client.attachment(message_id, &found).await?
            }
            Err(other) => return Err(other.into()),
        };
        let path = dir.join(format!("{id}.bin"));
        std::fs::write(&path, bytes)?;
        let path = path.to_string_lossy().into_owned();
        self.with_store(|s| Ok(s.set_attachment_cache(id, &path)?))?;
        Ok(())
    }
}
