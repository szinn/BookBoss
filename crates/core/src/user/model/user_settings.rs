use crate::user::UserId;

#[derive(Debug, Clone)]
pub struct UserSetting {
    pub user_id: UserId,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct NewUserSetting {
    pub user_id: UserId,
    pub key: String,
    pub value: String,
}
