use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::url::Url;

pub enum PermissionType {
    Unknown,
    AudioMedia,
    VideoMedia,
    DesktopMedia,
    MouseLock,
    DeviceInfo,
    Location,
    Clipboard,
    Notification,
}

impl From<saucer_permission_type> for PermissionType {
    fn from(value: saucer_permission_type) -> Self {
        match value {
            SAUCER_PERMISSION_TYPE_UNKNOWN => Self::Unknown,
            SAUCER_PERMISSION_TYPE_AUDIO_MEDIA => Self::AudioMedia,
            SAUCER_PERMISSION_TYPE_VIDEO_MEDIA => Self::VideoMedia,
            SAUCER_PERMISSION_TYPE_DESKTOP_MEDIA => Self::DesktopMedia,
            SAUCER_PERMISSION_TYPE_MOUSE_LOCK => Self::MouseLock,
            SAUCER_PERMISSION_TYPE_DEVICE_INFO => Self::DeviceInfo,
            SAUCER_PERMISSION_TYPE_LOCATION => Self::Location,
            SAUCER_PERMISSION_TYPE_CLIPBOARD => Self::Clipboard,
            SAUCER_PERMISSION_TYPE_NOTIFICATION => Self::Notification,
            _ => Self::Unknown,
        }
    }
}

pub struct PermissionRequest {
    inner: NonNull<saucer_permission_request>,
    _marker: PhantomData<saucer_permission_request>,
}

impl Drop for PermissionRequest {
    fn drop(&mut self) { unsafe { saucer_permission_request_free(self.inner.as_ptr()) } }
}

impl Clone for PermissionRequest {
    fn clone(&self) -> Self {
        unsafe { Self::from_ptr(saucer_permission_request_copy(self.inner.as_ptr())) }
    }
}

impl PermissionRequest {
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_permission_request) -> Self {
        Self {
            inner: NonNull::new(ptr).expect("invalid permission request ptr"),
            _marker: PhantomData,
        }
    }

    /// Sets whether to accept the permission request.
    pub fn accept(self, accept: bool) {
        unsafe { saucer_permission_request_accept(self.inner.as_ptr(), accept) };
    }

    /// Gets the requested permission type.
    pub fn kind(&self) -> PermissionType {
        unsafe { saucer_permission_request_type(self.inner.as_ptr()) }.into()
    }

    /// Gets the request URL.
    pub fn url(&self) -> Url {
        let ptr = unsafe { saucer_permission_request_url(self.inner.as_ptr()) };
        unsafe { Url::from_ptr(ptr, -1) }.expect("permission request URL should be present")
    }
}
