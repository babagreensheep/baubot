use baubot_core::prelude::BauData;

pub struct SqlLiteDb {}

impl BauData for SqlLiteDb {
    async fn register_user_chat_id(&self, username: &str) -> Option<i64> {
        todo!()
    }

    async fn insert_chat_id(&self, username: &str, chat_id: i64) -> Result<Option<i64>, String> {
        todo!()
    }

    async fn delete_chat_id(&self, username: &str) -> Result<i64, String> {
        todo!()
    }

    async fn get_chat_id(&self, username: &str) -> Option<i64> {
        todo!()
    }

    async fn is_admin(&self, username: &str) -> bool {
        todo!()
    }
}

#[cfg(feature = "test-utils")]
pub mod test_db;
