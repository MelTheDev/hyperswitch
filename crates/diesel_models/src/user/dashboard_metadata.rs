use common_utils::id_type;
use diesel::{query_builder::AsChangeset, Identifiable, Insertable, Queryable, Selectable};
use masking::Secret;
use time::PrimitiveDateTime;

use crate::{enums, schema::dashboard_metadata};

#[derive(Clone, Debug, Identifiable, Queryable, Selectable)]
#[diesel(table_name = dashboard_metadata, check_for_backend(diesel::pg::Pg))]
pub struct DashboardMetadata {
    pub id: i32,
    pub user_id: Option<String>,
    pub merchant_id: id_type::MerchantId,
    pub org_id: id_type::OrganizationId,
    pub data_key: enums::DashboardMetadata,
    pub data_value: Secret<serde_json::Value>,
    pub created_by: String,
    pub created_at: PrimitiveDateTime,
    pub last_modified_by: String,
    pub last_modified_at: PrimitiveDateTime,
}

#[derive(
    router_derive::Setter, Clone, Debug, Insertable, router_derive::DebugAsDisplay, AsChangeset,
)]
#[diesel(table_name = dashboard_metadata)]
pub struct DashboardMetadataNew {
    pub user_id: Option<String>,
    pub merchant_id: id_type::MerchantId,
    pub org_id: id_type::OrganizationId,
    pub data_key: enums::DashboardMetadata,
    pub data_value: Secret<serde_json::Value>,
    pub created_by: String,
    pub created_at: PrimitiveDateTime,
    pub last_modified_by: String,
    pub last_modified_at: PrimitiveDateTime,
}

#[derive(
    router_derive::Setter, Clone, Debug, Insertable, router_derive::DebugAsDisplay, AsChangeset,
)]
#[diesel(table_name = dashboard_metadata)]
pub struct DashboardMetadataUpdateInternal {
    pub data_key: enums::DashboardMetadata,
    pub data_value: Secret<serde_json::Value>,
    pub last_modified_by: String,
    pub last_modified_at: PrimitiveDateTime,
}

#[derive(Debug)]
pub enum DashboardMetadataUpdate {
    UpdateData {
        data_key: enums::DashboardMetadata,
        data_value: Secret<serde_json::Value>,
        last_modified_by: String,
    },
}

impl From<DashboardMetadataUpdate> for DashboardMetadataUpdateInternal {
    fn from(metadata_update: DashboardMetadataUpdate) -> Self {
        let last_modified_at = common_utils::date_time::now();
        match metadata_update {
            DashboardMetadataUpdate::UpdateData {
                data_key,
                data_value,
                last_modified_by,
            } => Self {
                data_key,
                data_value,
                last_modified_by,
                last_modified_at,
            },
        }
    }
}
