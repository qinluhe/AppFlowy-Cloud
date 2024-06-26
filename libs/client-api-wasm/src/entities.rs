use client_api::entity::AFUserProfile;
use client_api::error::{AppResponseError, ErrorCode};
use collab_entity::{CollabType, EncodedCollab};
use database_entity::dto::{
  AFUserWorkspaceInfo, AFWorkspace, BatchQueryCollabResult, PublishCollabMetadata, QueryCollab,
  QueryCollabParams, QueryCollabResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use tsify::Tsify;
use wasm_bindgen::JsValue;

macro_rules! from_struct_for_jsvalue {
  ($type:ty) => {
    impl From<$type> for JsValue {
      fn from(value: $type) -> Self {
        match serde_wasm_bindgen::to_value(&value) {
          Ok(js_value) => js_value,
          Err(err) => {
            tracing::error!("Failed to convert User to JsValue: {:?}", err);
            JsValue::NULL
          },
        }
      }
    }
  };
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Configuration {
  pub compression_quality: u32,
  pub compression_buffer_size: usize,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientAPIConfig {
  pub base_url: String,
  pub ws_addr: String,
  pub gotrue_url: String,
  pub device_id: String,
  pub configuration: Option<Configuration>,
  pub client_id: String,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientResponse {
  pub code: ErrorCode,
  pub message: String,
}

from_struct_for_jsvalue!(ClientResponse);
impl From<AppResponseError> for ClientResponse {
  fn from(err: AppResponseError) -> Self {
    ClientResponse {
      code: err.code,
      message: err.message.to_string(),
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct User {
  pub uid: String,
  pub uuid: String,
  pub email: Option<String>,
  pub name: Option<String>,
  pub latest_workspace_id: String,
  pub icon_url: Option<String>,
}

from_struct_for_jsvalue!(User);
impl From<AFUserProfile> for User {
  fn from(profile: AFUserProfile) -> Self {
    User {
      uid: profile.uid.to_string(),
      uuid: profile.uuid.to_string(),
      email: profile.email,
      name: profile.name,
      latest_workspace_id: profile.latest_workspace_id.to_string(),
      icon_url: None,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct UserWorkspace {
  pub user: User,
  pub visiting_workspace_id: String,
  pub workspaces: Vec<Workspace>,
}

from_struct_for_jsvalue!(UserWorkspace);

impl From<AFUserWorkspaceInfo> for UserWorkspace {
  fn from(info: AFUserWorkspaceInfo) -> Self {
    UserWorkspace {
      user: User::from(info.user_profile),
      visiting_workspace_id: info.visiting_workspace.workspace_id.to_string(),
      workspaces: info.workspaces.into_iter().map(Workspace::from).collect(),
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Workspace {
  pub workspace_id: String,
  pub database_storage_id: String,
  pub owner_uid: String,
  pub owner_name: String,
  pub workspace_type: i32,
  pub workspace_name: String,
  pub created_at: String,
  pub icon: String,
}

from_struct_for_jsvalue!(Workspace);

impl From<AFWorkspace> for Workspace {
  fn from(workspace: AFWorkspace) -> Self {
    Workspace {
      workspace_id: workspace.workspace_id.to_string(),
      database_storage_id: workspace.database_storage_id.to_string(),
      owner_uid: workspace.owner_uid.to_string(),
      owner_name: workspace.owner_name,
      workspace_type: workspace.workspace_type,
      workspace_name: workspace.workspace_name,
      created_at: workspace.created_at.timestamp().to_string(),
      icon: workspace.icon,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientQueryCollabParams {
  pub workspace_id: String,
  pub object_id: String,
  #[tsify(type = "0 | 1 | 2 | 3 | 4 | 5")]
  pub collab_type: i32,
}

impl From<ClientQueryCollabParams> for QueryCollabParams {
  fn from(value: ClientQueryCollabParams) -> QueryCollabParams {
    QueryCollabParams {
      workspace_id: value.workspace_id,
      inner: QueryCollab {
        collab_type: CollabType::from(value.collab_type),
        object_id: value.object_id,
      },
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientEncodeCollab {
  pub state_vector: Vec<u8>,
  pub doc_state: Vec<u8>,
  #[serde(default)]
  pub version: ClientEncoderVersion,
}

#[derive(Tsify, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ClientEncoderVersion {
  #[default]
  V1 = 0,
  V2 = 1,
}

from_struct_for_jsvalue!(ClientEncodeCollab);

impl From<EncodedCollab> for ClientEncodeCollab {
  fn from(collab: EncodedCollab) -> Self {
    ClientEncodeCollab {
      state_vector: collab.state_vector.to_vec(),
      doc_state: collab.doc_state.to_vec(),
      version: ClientEncoderVersion::V1,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct BatchClientQueryCollab(pub Vec<ClientQueryCollab>);
#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientQueryCollab {
  pub object_id: String,
  #[tsify(type = "0 | 1 | 2 | 3 | 4 | 5")]
  pub collab_type: i32,
}

from_struct_for_jsvalue!(ClientQueryCollab);

impl From<ClientQueryCollab> for QueryCollab {
  fn from(value: ClientQueryCollab) -> QueryCollab {
    QueryCollab {
      collab_type: CollabType::from(value.collab_type),
      object_id: value.object_id,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct BatchClientEncodeCollab(pub HashMap<String, ClientEncodeCollab>);

from_struct_for_jsvalue!(BatchClientEncodeCollab);

impl From<BatchQueryCollabResult> for BatchClientEncodeCollab {
  fn from(result: BatchQueryCollabResult) -> Self {
    let mut hash_map = HashMap::new();

    result.0.into_iter().for_each(|(k, v)| match v {
      QueryCollabResult::Success { encode_collab_v1 } => {
        EncodedCollab::decode_from_bytes(&encode_collab_v1)
          .map(|collab| {
            hash_map.insert(k, ClientEncodeCollab::from(collab));
          })
          .unwrap_or_else(|err| {
            tracing::error!("Failed to decode collab: {:?}", err);
          });
      },
      QueryCollabResult::Failed { .. } => {
        tracing::error!("Failed to get collab: {:?}", k);
      },
    });

    BatchClientEncodeCollab(hash_map)
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishViewMeta {
  pub view_id: String,
  pub publish_name: String,
  pub metadata: PublishViewMetaData,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishViewMetaData {
  pub view: PublishViewInfo,
  pub child_views: Vec<PublishViewInfo>,
  pub ancestor_views: Vec<PublishViewInfo>,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishViewInfo {
  pub view_id: String,
  pub name: String,
  pub icon: Option<String>,
  pub layout: i32,
  pub extra: Option<String>,
  pub created_by: Option<String>,
  pub last_edited_by: Option<String>,
  pub last_edited_time: String,
  pub created_at: String,
  pub child_views: Option<Vec<PublishViewInfo>>,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishViewPayload {
  pub meta: PublishViewMeta,
  /// The doc_state of the encoded collab.
  pub data: Vec<u8>,
}
#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishInfo {
  pub namespace: Option<String>,
  pub publish_name: String,
}

from_struct_for_jsvalue!(PublishViewMeta);
from_struct_for_jsvalue!(PublishViewMetaData);
from_struct_for_jsvalue!(PublishViewPayload);
from_struct_for_jsvalue!(PublishInfo);

impl From<PublishCollabMetadata<serde_json::Value>> for PublishViewMeta {
  fn from(value: PublishCollabMetadata<Value>) -> Self {
    let view_id = value.view_id.to_string();
    let publish_name = value.publish_name.to_string();
    let metadata = PublishViewMetaData::from(value.metadata);
    Self {
      view_id,
      publish_name,
      metadata,
    }
  }
}

impl From<serde_json::Value> for PublishViewMetaData {
  fn from(value: Value) -> Self {
    let view = PublishViewInfo::from(value["view"].clone());
    let child_views = parse_views(&value["child_views"]);
    let ancestor_views = parse_views(&value["ancestor_views"]);
    Self {
      view,
      child_views: child_views.unwrap_or_default(),
      ancestor_views: ancestor_views.unwrap_or_default(),
    }
  }
}

fn match_serde_value_to_string(value: &Value) -> Option<String> {
  match value {
    Value::String(v) => Some(v.to_string()),
    Value::Null => None,
    v => Some(v.to_string()),
  }
}

impl From<serde_json::Value> for PublishViewInfo {
  fn from(value: Value) -> Self {
    let view_id = match_serde_value_to_string(&value["view_id"]).unwrap_or_default();
    let name = match_serde_value_to_string(&value["name"]).unwrap_or_default();
    let icon = match_serde_value_to_string(&value["icon"]);
    let layout = value["layout"]
      .as_i64()
      .map(|v| v as i32)
      .unwrap_or_default();
    let extra = match_serde_value_to_string(&value["extra"]);
    let created_by = match_serde_value_to_string(&value["created_by"]);
    let last_edited_by = match_serde_value_to_string(&value["last_edited_by"]);
    let last_edited_time =
      match_serde_value_to_string(&value["last_edited_time"]).unwrap_or_default();
    let created_at = match_serde_value_to_string(&value["created_at"]).unwrap_or_default();
    let child_views = parse_views(&value["child_views"]);
    Self {
      view_id,
      name,
      icon,
      layout,
      extra,
      created_by,
      last_edited_by,
      last_edited_time,
      created_at,
      child_views,
    }
  }
}

fn parse_views(value: &Value) -> Option<Vec<PublishViewInfo>> {
  value.as_array().map(|v| {
    v.iter()
      .map(|v| PublishViewInfo::from(v.clone()))
      .collect::<Vec<_>>()
  })
}
