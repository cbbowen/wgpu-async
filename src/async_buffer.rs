use crate::async_device::AsyncDevice;
use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut, RangeBounds},
};
use wgpu::BufferAddress;

/// A wrapper around a [`wgpu::Buffer`] which shadows some methods to allow for async
/// mapping using Rust's `async` API.
#[derive(Debug)]
pub struct AsyncBuffer
where
    Self: wgpu::WasmNotSend,
{
    device: AsyncDevice,
    buffer: wgpu::Buffer,
}

impl AsyncBuffer {
    /// Wraps a buffer to allow for mapping using `async`.
    pub fn wrap(device: AsyncDevice, buffer: wgpu::Buffer) -> Self {
        Self { device, buffer }
    }

    /// Takes a slice of this buffer, in the same way a call to [`wgpu::Buffer::slice`] would,
    /// except wraps the result in an [`AsyncBufferSlice`] so that the `map_async` method can be
    /// awaited.
    pub fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> AsyncBufferSlice<'_> {
        let buffer_slice = self.buffer.slice(bounds);
        AsyncBufferSlice {
            device: self.device.clone(),
            buffer_slice,
        }
    }

    /// An awaitable version of [`wgpu::Buffer::map_async`] with [`wgpu::MapMode::Read`].
    pub async fn map_async<S: RangeBounds<wgpu::BufferAddress>>(
        &self,
        bounds: S,
    ) -> Result<AsyncBufferView<'_>, wgpu::BufferAsyncError> {
        let slice = self.slice(bounds);
        slice.map_async().await
    }

    /// An awaitable version of [`wgpu::Buffer::map_async`] with [`wgpu::MapMode::Write`].
    pub async fn map_async_mut<S: RangeBounds<wgpu::BufferAddress>>(
        &self,
        bounds: S,
    ) -> Result<AsyncBufferViewMut<'_>, wgpu::BufferAsyncError> {
        let slice = self.slice(bounds);
        slice.map_async_mut().await
    }
}
impl Deref for AsyncBuffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl DerefMut for AsyncBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}
impl<T> AsRef<T> for AsyncBuffer
where
    T: ?Sized,
    <AsyncBuffer as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}
impl<T> AsMut<T> for AsyncBuffer
where
    <AsyncBuffer as Deref>::Target: AsMut<T>,
{
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut().as_mut()
    }
}

pub struct AsyncBufferView<'a> {
    buffer: &'a wgpu::Buffer,
    buffer_view: ManuallyDrop<wgpu::BufferView>,
}

impl<'a> Deref for AsyncBufferView<'a> {
    type Target = wgpu::BufferView;
    fn deref(&self) -> &Self::Target {
        &self.buffer_view
    }
}

impl<'a> AsyncBufferView<'a> {
    fn new(buffer_slice: &wgpu::BufferSlice<'a>) -> Self {
        let buffer_view = buffer_slice.get_mapped_range();
        Self {
            buffer: buffer_slice.buffer(),
            buffer_view: ManuallyDrop::new(buffer_view),
        }
    }
}

impl<'a> Drop for AsyncBufferView<'a> {
    fn drop(&mut self) {
        // `buffer_view` is never used after this point.
        unsafe {
            ManuallyDrop::drop(&mut self.buffer_view);
        }

        self.buffer.unmap();
    }
}

pub struct AsyncBufferViewMut<'a> {
    buffer: &'a wgpu::Buffer,
    buffer_view: ManuallyDrop<wgpu::BufferViewMut>,
}

impl<'a> Deref for AsyncBufferViewMut<'a> {
    type Target = wgpu::BufferViewMut;
    fn deref(&self) -> &Self::Target {
        &self.buffer_view
    }
}

impl<'a> DerefMut for AsyncBufferViewMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer_view
    }
}

impl<'a> AsyncBufferViewMut<'a> {
    fn new(buffer_slice: &wgpu::BufferSlice<'a>) -> Self {
        let buffer_view = buffer_slice.get_mapped_range_mut();
        Self {
            buffer: buffer_slice.buffer(),
            buffer_view: ManuallyDrop::new(buffer_view),
        }
    }
}

impl<'a> Drop for AsyncBufferViewMut<'a> {
    fn drop(&mut self) {
        // `buffer_view` is never used after this point.
        unsafe {
            ManuallyDrop::drop(&mut self.buffer_view);
        }

        self.buffer.unmap();
    }
}

/// A smart-pointer wrapper around a [`wgpu::BufferSlice`], offering a `map_async` method than can be `await`ed.
#[derive(Debug)]
pub struct AsyncBufferSlice<'a>
where
    Self: wgpu::WasmNotSend,
{
    device: AsyncDevice,
    buffer_slice: wgpu::BufferSlice<'a>,
}
impl<'a> AsyncBufferSlice<'a> {
    /// Wraps a buffer slice to allow for mapping using `async`.
    pub fn wrap(device: AsyncDevice, buffer_slice: wgpu::BufferSlice<'a>) -> Self {
        Self {
            device,
            buffer_slice,
        }
    }

    /// An awaitable version of [`wgpu::BufferSlice::map_async`] with [`wgpu::MapMode::Read`].
    pub async fn map_async(self) -> Result<AsyncBufferView<'a>, wgpu::BufferAsyncError> {
        self.device
            .do_async(|callback| self.buffer_slice.map_async(wgpu::MapMode::Read, callback))
            .await?;
        Ok(AsyncBufferView::new(&self.buffer_slice))
    }

    /// An awaitable version of [`wgpu::BufferSlice::map_async`] with [`wgpu::MapMode::Write`].
    pub async fn map_async_mut(self) -> Result<AsyncBufferViewMut<'a>, wgpu::BufferAsyncError> {
        self.device
            .do_async(|callback| self.buffer_slice.map_async(wgpu::MapMode::Write, callback))
            .await?;
        Ok(AsyncBufferViewMut::new(&self.buffer_slice))
    }
}
impl<'a> Deref for AsyncBufferSlice<'a> {
    type Target = wgpu::BufferSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.buffer_slice
    }
}
impl DerefMut for AsyncBufferSlice<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer_slice
    }
}
impl<T> AsRef<T> for AsyncBufferSlice<'_>
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}
impl<T> AsMut<T> for AsyncBufferSlice<'_>
where
    <Self as Deref>::Target: AsMut<T>,
{
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut().as_mut()
    }
}
