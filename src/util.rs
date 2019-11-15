use crate::error::{Error, Result};
use crate::resource::{DriveId, ItemId};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{de, Deserialize};
use url::PathSegmentsMut;

/// Specify the location of a `Drive` resource.
///
/// # See also
/// [`resource::Drive`][drive]
///
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0)
///
/// [drive]: ./resource/struct.Drive.html
#[derive(Clone, Debug)]
pub struct DriveLocation {
    inner: DriveLocationEnum,
}

#[derive(Clone, Debug)]
enum DriveLocationEnum {
    Me,
    User(String),
    Group(String),
    Site(String),
    Id(DriveId),
}

impl DriveLocation {
    /// Current user's OneDrive.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-current-users-onedrive)
    pub fn me() -> Self {
        Self {
            inner: DriveLocationEnum::Me,
        }
    }

    /// OneDrive of a user.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-a-users-onedrive)
    pub fn from_user(id_or_principal_name: String) -> Self {
        Self {
            inner: DriveLocationEnum::User(id_or_principal_name),
        }
    }

    /// The document library associated with a group.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-the-document-library-associated-with-a-group)
    pub fn from_group(group_id: String) -> Self {
        Self {
            inner: DriveLocationEnum::Group(group_id),
        }
    }

    /// The document library for a site.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-the-document-library-for-a-site)
    pub fn from_site(site_id: String) -> Self {
        Self {
            inner: DriveLocationEnum::Site(site_id),
        }
    }

    /// A drive with ID specified.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-a-drive-by-id)
    pub fn from_id(drive_id: DriveId) -> Self {
        Self {
            inner: DriveLocationEnum::Id(drive_id),
        }
    }
}

impl From<DriveId> for DriveLocation {
    fn from(id: DriveId) -> Self {
        Self::from_id(id)
    }
}

/// Reference to a `DriveItem` in a drive.
/// It does not contains the drive information.
///
/// # See also
/// [`resource::DriveItem`][drive_item]
///
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-get?view=graph-rest-1.0)
///
/// [drive_item]: ./resource/struct.DriveItem.html
#[derive(Clone, Copy, Debug)]
pub struct ItemLocation<'a> {
    inner: ItemLocationEnum<'a>,
}

#[derive(Clone, Copy, Debug)]
enum ItemLocationEnum<'a> {
    Path(&'a str),
    Id(&'a str),
}

impl<'a> ItemLocation<'a> {
    /// A UNIX-like `/`-started absolute path to a file or directory in the drive.
    ///
    /// # Error
    /// If `path` contains invalid characters for OneDrive API, it returns None.
    ///
    /// # Note
    /// The trailing `/` is optional.
    ///
    /// Special name on Windows like `CON` or `NUL` is tested to be permitted in API,
    /// but may still cause errors on Windows or OneDrive Online.
    /// These names will pass the check, but STRONGLY NOT recommended.
    ///
    /// # See also
    /// [Microsoft Docs](https://support.office.com/en-us/article/Invalid-file-names-and-file-types-in-OneDrive-OneDrive-for-Business-and-SharePoint-64883a5d-228e-48f5-b3d2-eb39e07630fa#invalidcharacters)
    pub fn from_path(path: &'a str) -> Option<Self> {
        if path == "/" {
            Some(Self::root())
        } else if path.starts_with('/')
            && path[1..]
                .split_terminator('/')
                .all(|comp| !comp.is_empty() && FileName::new(comp).is_some())
        {
            Some(Self {
                inner: ItemLocationEnum::Path(path),
            })
        } else {
            None
        }
    }

    /// Item id from other API.
    pub fn from_id(item_id: &'a ItemId) -> Self {
        Self {
            inner: ItemLocationEnum::Id(item_id.as_ref()),
        }
    }

    /// The root directory item.
    pub fn root() -> Self {
        Self {
            inner: ItemLocationEnum::Path("/"),
        }
    }
}

impl<'a> From<&'a ItemId> for ItemLocation<'a> {
    fn from(id: &'a ItemId) -> Self {
        Self::from_id(id)
    }
}

/// An valid file name str (unsized).
#[derive(Debug)]
pub struct FileName(str);

impl FileName {
    /// Check and wrap the name for a file or a directory in OneDrive.
    ///
    /// Returns None if contains invalid characters.
    ///
    /// # See also
    /// [ItemLocation::from_path][from_path]
    ///
    /// [from_path]: ../struct.ItemLocation.html#method.from_path
    pub fn new<S: AsRef<str> + ?Sized>(name: &S) -> Option<&Self> {
        const INVALID_CHARS: &str = r#""*:<>?/\|"#;

        let name = name.as_ref();
        if !name.is_empty() && !name.contains(|c| INVALID_CHARS.contains(c)) {
            Some(unsafe { &*(name as *const str as *const Self) })
        } else {
            None
        }
    }

    /// View the file name as `&str`. It is cost-free.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for FileName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

pub(crate) trait ApiPathComponent {
    fn extend_into(&self, buf: &mut PathSegmentsMut);
}

impl ApiPathComponent for DriveLocation {
    fn extend_into(&self, buf: &mut PathSegmentsMut) {
        use self::DriveLocationEnum::*;
        match &self.inner {
            Me => buf.push("drive"),
            User(id) => buf.extend(&["users", id, "drive"]),
            Group(id) => buf.extend(&["groups", id, "drive"]),
            Site(id) => buf.extend(&["sites", id, "drive"]),
            Id(id) => buf.extend(&["drives", id.as_ref()]),
        };
    }
}

impl ApiPathComponent for ItemLocation<'_> {
    fn extend_into(&self, buf: &mut PathSegmentsMut) {
        use self::ItemLocationEnum::*;
        match &self.inner {
            Path("/") => buf.push("root"),
            Path(path) => buf.push(&["root:", path, ":"].join("")),
            Id(id) => buf.extend(&["items", id]),
        };
    }
}

impl ApiPathComponent for str {
    fn extend_into(&self, buf: &mut PathSegmentsMut) {
        buf.push(self);
    }
}

pub(crate) trait RequestBuilderTransformer {
    fn trans(&self, req: RequestBuilder) -> RequestBuilder;
}

pub(crate) trait RequestBuilderExt: Sized {
    fn opt_header(self, key: impl AsRef<str>, value: Option<impl AsRef<str>>) -> Self;
    fn apply(self, f: &impl RequestBuilderTransformer) -> Self;
}

impl RequestBuilderExt for RequestBuilder {
    fn opt_header(self, key: impl AsRef<str>, value: Option<impl AsRef<str>>) -> Self {
        match value {
            Some(v) => self.header(key.as_ref(), v.as_ref()),
            None => self,
        }
    }

    fn apply(self, f: &impl RequestBuilderTransformer) -> Self {
        f.trans(self)
    }
}

pub(crate) trait ResponseExt: Sized {
    fn check_status(self) -> Result<Self>;
    fn parse<T: de::DeserializeOwned>(self) -> Result<T>;
    fn parse_optional<T: de::DeserializeOwned>(self) -> Result<Option<T>>;
    fn parse_no_content(self) -> Result<()>;
}

impl ResponseExt for Response {
    fn check_status(mut self) -> Result<Self> {
        match self.error_for_status_ref() {
            Ok(_) => Ok(self),
            Err(source) => {
                #[derive(Deserialize)]
                struct ErrorResponse {
                    error: crate::resource::ErrorObject,
                }

                let response: ErrorResponse = self.json()?; // Throw network or serialization error first.
                Err(Error::from_response(source, Some(response.error)))
            }
        }
    }

    fn parse<T: de::DeserializeOwned>(self) -> Result<T> {
        Ok(self.check_status()?.json()?)
    }

    fn parse_optional<T: de::DeserializeOwned>(self) -> Result<Option<T>> {
        match self.status() {
            StatusCode::NOT_MODIFIED | StatusCode::ACCEPTED => Ok(None),
            _ => Ok(Some(self.parse()?)),
        }
    }

    fn parse_no_content(self) -> Result<()> {
        self.check_status()?;
        Ok(())
    }
}