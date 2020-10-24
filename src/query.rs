// Copyright 2019 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::archetype::Archetype;
use crate::entities::EntityMeta;
use crate::{Component, Entity};

/// A collection of component types to fetch from a `World`
pub trait Query {
    #[doc(hidden)]
    type Fetch: for<'a> Fetch<'a>;
}

/// Streaming iterators over contiguous homogeneous ranges of components
pub trait Fetch<'a>: Sized {
    /// Type of value to be fetched
    type Item;

    /// A value on which `get` may never be called
    #[allow(clippy::declare_interior_mutable_const)] // no const fn in traits
    const DANGLING: Self;

    /// How this query will access `archetype`, if at all
    fn access(archetype: &Archetype) -> Option<Access>;

    /// Acquire dynamic borrows from `archetype`
    fn borrow(archetype: &Archetype);
    /// Construct a `Fetch` for `archetype` if it should be traversed
    fn new(archetype: &'a Archetype) -> Option<Self>;
    /// Release dynamic borrows acquired by `borrow`
    fn release(archetype: &Archetype);

    /// Access the `n`th item in this archetype without bounds checking
    ///
    /// # Safety
    /// - Must only be called after `borrow`
    /// - `release` must not be called while `'a` is still live
    /// - Bounds-checking must be performed externally
    /// - Any resulting borrows must be legal (e.g. no &mut to something another iterator might access)
    unsafe fn get(&self, n: usize) -> Self::Item;
}

/// Type of access a `Query` may have to an `Archetype`
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Access {
    /// Read entity IDs only, no components
    Iterate,
    /// Read components
    Read,
    /// Read and write components
    Write,
}

impl<'a, T: Component> Query for &'a T {
    type Fetch = FetchRead<T>;
}

#[doc(hidden)]
pub struct FetchRead<T>(NonNull<T>);

impl<'a, T: Component> Fetch<'a> for FetchRead<T> {
    type Item = &'a T;

    const DANGLING: Self = Self(NonNull::dangling());

    fn access(archetype: &Archetype) -> Option<Access> {
        if archetype.has::<T>() {
            Some(Access::Read)
        } else {
            None
        }
    }

    fn borrow(archetype: &Archetype) {
        archetype.borrow::<T>();
    }
    fn new(archetype: &'a Archetype) -> Option<Self> {
        archetype.get::<T>().map(Self)
    }
    fn release(archetype: &Archetype) {
        archetype.release::<T>();
    }

    unsafe fn get(&self, n: usize) -> Self::Item {
        &*self.0.as_ptr().add(n)
    }
}

impl<'a, T: Component> Query for &'a mut T {
    type Fetch = FetchWrite<T>;
}

#[doc(hidden)]
pub struct FetchWrite<T>(NonNull<T>);

impl<'a, T: Component> Fetch<'a> for FetchWrite<T> {
    type Item = &'a mut T;

    const DANGLING: Self = Self(NonNull::dangling());

    fn access(archetype: &Archetype) -> Option<Access> {
        if archetype.has::<T>() {
            Some(Access::Write)
        } else {
            None
        }
    }

    fn borrow(archetype: &Archetype) {
        archetype.borrow_mut::<T>();
    }
    fn new(archetype: &'a Archetype) -> Option<Self> {
        archetype.get::<T>().map(Self)
    }
    fn release(archetype: &Archetype) {
        archetype.release_mut::<T>();
    }

    unsafe fn get(&self, n: usize) -> Self::Item {
        &mut *self.0.as_ptr().add(n)
    }
}

impl<T: Query> Query for Option<T> {
    type Fetch = TryFetch<T::Fetch>;
}

#[doc(hidden)]
pub struct TryFetch<T>(Option<T>);

impl<'a, T: Fetch<'a>> Fetch<'a> for TryFetch<T> {
    type Item = Option<T::Item>;

    const DANGLING: Self = Self(None);

    fn access(archetype: &Archetype) -> Option<Access> {
        Some(T::access(archetype).unwrap_or(Access::Iterate))
    }

    fn borrow(archetype: &Archetype) {
        T::borrow(archetype)
    }
    fn new(archetype: &'a Archetype) -> Option<Self> {
        Some(Self(T::new(archetype)))
    }
    fn release(archetype: &Archetype) {
        T::release(archetype)
    }

    unsafe fn get(&self, n: usize) -> Option<T::Item> {
        Some(self.0.as_ref()?.get(n))
    }
}

/// Query transformer skipping entities that have a `T` component
///
/// See also `QueryBorrow::without`.
///
/// # Example
/// ```
/// # use hecs::*;
/// let mut world = World::new();
/// let a = world.spawn((123, true, "abc"));
/// let b = world.spawn((456, false));
/// let c = world.spawn((42, "def"));
/// let entities = world.query::<Without<bool, &i32>>()
///     .iter()
///     .map(|(e, &i)| (e, i))
///     .collect::<Vec<_>>();
/// assert_eq!(entities, &[(c, 42)]);
/// ```
pub struct Without<T, Q>(PhantomData<(Q, fn(T))>);

impl<T: Component, Q: Query> Query for Without<T, Q> {
    type Fetch = FetchWithout<T, Q::Fetch>;
}

#[doc(hidden)]
pub struct FetchWithout<T, F>(F, PhantomData<fn(T)>);

impl<'a, T: Component, F: Fetch<'a>> Fetch<'a> for FetchWithout<T, F> {
    type Item = F::Item;

    const DANGLING: Self = Self(F::DANGLING, PhantomData);

    fn access(archetype: &Archetype) -> Option<Access> {
        if archetype.has::<T>() {
            None
        } else {
            F::access(archetype)
        }
    }

    fn borrow(archetype: &Archetype) {
        F::borrow(archetype)
    }
    fn new(archetype: &'a Archetype) -> Option<Self> {
        if archetype.has::<T>() {
            return None;
        }
        Some(Self(F::new(archetype)?, PhantomData))
    }
    fn release(archetype: &Archetype) {
        F::release(archetype)
    }

    unsafe fn get(&self, n: usize) -> F::Item {
        self.0.get(n)
    }
}

/// Query transformer skipping entities that do not have a `T` component
///
/// See also `QueryBorrow::with`.
///
/// # Example
/// ```
/// # use hecs::*;
/// let mut world = World::new();
/// let a = world.spawn((123, true, "abc"));
/// let b = world.spawn((456, false));
/// let c = world.spawn((42, "def"));
/// let entities = world.query::<With<bool, &i32>>()
///     .iter()
///     .map(|(e, &i)| (e, i))
///     .collect::<Vec<_>>();
/// assert_eq!(entities.len(), 2);
/// assert!(entities.contains(&(a, 123)));
/// assert!(entities.contains(&(b, 456)));
/// ```
pub struct With<T, Q>(PhantomData<(Q, fn(T))>);

impl<T: Component, Q: Query> Query for With<T, Q> {
    type Fetch = FetchWith<T, Q::Fetch>;
}

#[doc(hidden)]
pub struct FetchWith<T, F>(F, PhantomData<fn(T)>);

impl<'a, T: Component, F: Fetch<'a>> Fetch<'a> for FetchWith<T, F> {
    type Item = F::Item;

    const DANGLING: Self = Self(F::DANGLING, PhantomData);

    fn access(archetype: &Archetype) -> Option<Access> {
        if archetype.has::<T>() {
            F::access(archetype)
        } else {
            None
        }
    }

    fn borrow(archetype: &Archetype) {
        F::borrow(archetype)
    }
    fn new(archetype: &'a Archetype) -> Option<Self> {
        if !archetype.has::<T>() {
            return None;
        }
        Some(Self(F::new(archetype)?, PhantomData))
    }
    fn release(archetype: &Archetype) {
        F::release(archetype)
    }

    unsafe fn get(&self, n: usize) -> F::Item {
        self.0.get(n)
    }
}

/// A borrow of a `World` sufficient to execute the query `Q`
///
/// Note that borrows are not released until this object is dropped.
pub struct QueryBorrow<'w, Q: Query> {
    meta: &'w [EntityMeta],
    archetypes: &'w [Archetype],
    borrowed: bool,
    _marker: PhantomData<Q>,
}

impl<'w, Q: Query> QueryBorrow<'w, Q> {
    pub(crate) fn new(meta: &'w [EntityMeta], archetypes: &'w [Archetype]) -> Self {
        Self {
            meta,
            archetypes,
            borrowed: false,
            _marker: PhantomData,
        }
    }

    /// Execute the query
    ///
    /// Must be called only once per query.
    pub fn iter<'q>(&'q mut self) -> QueryIter<'q, 'w, Q> {
        self.borrow();
        QueryIter {
            borrow: self,
            archetype_index: 0,
            iter: ChunkIter::EMPTY,
        }
    }

    /// Like `iter`, but returns child iterators of at most `batch_size` elements
    ///
    /// Useful for distributing work over a threadpool.
    pub fn iter_batched<'q>(&'q mut self, batch_size: u32) -> BatchedIter<'q, 'w, Q> {
        self.borrow();
        BatchedIter {
            borrow: self,
            archetype_index: 0,
            batch_size,
            batch: 0,
        }
    }

    fn borrow(&mut self) {
        if self.borrowed {
            panic!(
                "called QueryBorrow::iter twice on the same borrow; construct a new query instead"
            );
        }
        for x in self.archetypes {
            // TODO: Release prior borrows on failure?
            if Q::Fetch::access(x) >= Some(Access::Read) {
                Q::Fetch::borrow(x);
            }
        }
        self.borrowed = true;
    }

    /// Transform the query into one that requires a certain component without borrowing it
    ///
    /// This can be useful when the component needs to be borrowed elsewhere and it isn't necessary
    /// for the iterator to expose its data directly.
    ///
    /// Equivalent to using a query type wrapped in `With`.
    ///
    /// # Example
    /// ```
    /// # use hecs::*;
    /// let mut world = World::new();
    /// let a = world.spawn((123, true, "abc"));
    /// let b = world.spawn((456, false));
    /// let c = world.spawn((42, "def"));
    /// let entities = world.query::<&i32>()
    ///     .with::<bool>()
    ///     .iter()
    ///     .map(|(e, &i)| (e, i)) // Copy out of the world
    ///     .collect::<Vec<_>>();
    /// assert!(entities.contains(&(a, 123)));
    /// assert!(entities.contains(&(b, 456)));
    /// ```
    pub fn with<T: Component>(self) -> QueryBorrow<'w, With<T, Q>> {
        self.transform()
    }

    /// Transform the query into one that skips entities having a certain component
    ///
    /// Equivalent to using a query type wrapped in `Without`.
    ///
    /// # Example
    /// ```
    /// # use hecs::*;
    /// let mut world = World::new();
    /// let a = world.spawn((123, true, "abc"));
    /// let b = world.spawn((456, false));
    /// let c = world.spawn((42, "def"));
    /// let entities = world.query::<&i32>()
    ///     .without::<bool>()
    ///     .iter()
    ///     .map(|(e, &i)| (e, i)) // Copy out of the world
    ///     .collect::<Vec<_>>();
    /// assert_eq!(entities, &[(c, 42)]);
    /// ```
    pub fn without<T: Component>(self) -> QueryBorrow<'w, Without<T, Q>> {
        self.transform()
    }

    /// Helper to change the type of the query
    fn transform<R: Query>(mut self) -> QueryBorrow<'w, R> {
        let x = QueryBorrow {
            meta: self.meta,
            archetypes: self.archetypes,
            borrowed: self.borrowed,
            _marker: PhantomData,
        };
        // Ensure `Drop` won't fire redundantly
        self.borrowed = false;
        x
    }
}

unsafe impl<'w, Q: Query> Send for QueryBorrow<'w, Q> {}
unsafe impl<'w, Q: Query> Sync for QueryBorrow<'w, Q> {}

impl<'w, Q: Query> Drop for QueryBorrow<'w, Q> {
    fn drop(&mut self) {
        if self.borrowed {
            for x in self.archetypes {
                if Q::Fetch::access(x) >= Some(Access::Read) {
                    Q::Fetch::release(x);
                }
            }
        }
    }
}

impl<'q, 'w, Q: Query> IntoIterator for &'q mut QueryBorrow<'w, Q> {
    type Item = (Entity, <Q::Fetch as Fetch<'q>>::Item);
    type IntoIter = QueryIter<'q, 'w, Q>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the set of entities with the components in `Q`
pub struct QueryIter<'q, 'w, Q: Query> {
    borrow: &'q mut QueryBorrow<'w, Q>,
    archetype_index: usize,
    iter: ChunkIter<Q>,
}

unsafe impl<'q, 'w, Q: Query> Send for QueryIter<'q, 'w, Q> {}
unsafe impl<'q, 'w, Q: Query> Sync for QueryIter<'q, 'w, Q> {}

impl<'q, 'w, Q: Query> Iterator for QueryIter<'q, 'w, Q> {
    type Item = (Entity, <Q::Fetch as Fetch<'q>>::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match unsafe { self.iter.next() } {
                None => {
                    let archetype = self.borrow.archetypes.get(self.archetype_index)?;
                    self.archetype_index += 1;
                    self.iter =
                        Q::Fetch::new(archetype).map_or(ChunkIter::EMPTY, |fetch| ChunkIter {
                            entities: archetype.entities(),
                            fetch,
                            position: 0,
                            len: archetype.len() as usize,
                        });
                    continue;
                }
                Some((id, components)) => {
                    return Some((
                        Entity {
                            id,
                            generation: unsafe {
                                self.borrow.meta.get_unchecked(id as usize).generation
                            },
                        },
                        components,
                    ));
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}

impl<'q, 'w, Q: Query> ExactSizeIterator for QueryIter<'q, 'w, Q> {
    fn len(&self) -> usize {
        self.borrow
            .archetypes
            .iter()
            .filter(|&x| Q::Fetch::access(x).is_some())
            .map(|x| x.len() as usize)
            .sum()
    }
}

struct ChunkIter<Q: Query> {
    entities: NonNull<u32>,
    fetch: Q::Fetch,
    position: usize,
    len: usize,
}

impl<Q: Query> ChunkIter<Q> {
    #[allow(clippy::declare_interior_mutable_const)] // no trait bounds on const fns
    const EMPTY: Self = Self {
        entities: NonNull::dangling(),
        fetch: Q::Fetch::DANGLING,
        position: 0,
        len: 0,
    };

    #[inline]
    unsafe fn next<'a>(&mut self) -> Option<(u32, <Q::Fetch as Fetch<'a>>::Item)> {
        if self.position == self.len {
            return None;
        }
        let entity = self.entities.as_ptr().add(self.position);
        let item = self.fetch.get(self.position);
        self.position += 1;
        Some((*entity, item))
    }
}

/// Batched version of `QueryIter`
pub struct BatchedIter<'q, 'w, Q: Query> {
    borrow: &'q mut QueryBorrow<'w, Q>,
    archetype_index: usize,
    batch_size: u32,
    batch: u32,
}

unsafe impl<'q, 'w, Q: Query> Send for BatchedIter<'q, 'w, Q> {}
unsafe impl<'q, 'w, Q: Query> Sync for BatchedIter<'q, 'w, Q> {}

impl<'q, 'w, Q: Query> Iterator for BatchedIter<'q, 'w, Q> {
    type Item = Batch<'q, 'w, Q>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let archetype = self.borrow.archetypes.get(self.archetype_index)?;
            let offset = self.batch_size * self.batch;
            if offset >= archetype.len() {
                self.archetype_index += 1;
                self.batch = 0;
                continue;
            }
            if let Some(fetch) = Q::Fetch::new(archetype) {
                self.batch += 1;
                return Some(Batch {
                    _marker: PhantomData,
                    meta: self.borrow.meta,
                    state: ChunkIter {
                        entities: archetype.entities(),
                        fetch,
                        len: (offset + self.batch_size.min(archetype.len() - offset)) as usize,
                        position: offset as usize,
                    },
                });
            } else {
                self.archetype_index += 1;
                debug_assert_eq!(
                    self.batch, 0,
                    "query fetch should always reject at the first batch or not at all"
                );
                continue;
            }
        }
    }
}

/// A sequence of entities yielded by `BatchedIter`
pub struct Batch<'q, 'w, Q: Query> {
    _marker: PhantomData<&'q ()>,
    meta: &'w [EntityMeta],
    state: ChunkIter<Q>,
}

impl<'q, 'w, Q: Query> Iterator for Batch<'q, 'w, Q> {
    type Item = (Entity, <Q::Fetch as Fetch<'q>>::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let (id, components) = unsafe { self.state.next()? };
        Some((
            Entity {
                id,
                generation: self.meta[id as usize].generation,
            },
            components,
        ))
    }
}

unsafe impl<'q, 'w, Q: Query> Send for Batch<'q, 'w, Q> {}
unsafe impl<'q, 'w, Q: Query> Sync for Batch<'q, 'w, Q> {}

macro_rules! tuple_impl {
    ($($name: ident),*) => {
        impl<'a, $($name: Fetch<'a>),*> Fetch<'a> for ($($name,)*) {
            type Item = ($($name::Item,)*);

            const DANGLING: Self = ($($name::DANGLING,)*);

            #[allow(unused_variables, unused_mut)]
            fn access(archetype: &Archetype) -> Option<Access> {
                let mut access = Access::Iterate;
                $(
                    access = access.max($name::access(archetype)?);
                )*
                Some(access)
            }

            #[allow(unused_variables)]
            fn borrow(archetype: &Archetype) {
                $($name::borrow(archetype);)*
            }
            #[allow(unused_variables)]
            fn new(archetype: &'a Archetype) -> Option<Self> {
                Some(($($name::new(archetype)?,)*))
            }
            #[allow(unused_variables)]
            fn release(archetype: &Archetype) {
                $($name::release(archetype);)*
            }

            #[allow(unused_variables)]
            unsafe fn get(&self, n: usize) -> Self::Item {
                #[allow(non_snake_case)]
                let ($($name,)*) = self;
                ($($name.get(n),)*)
            }
        }

        impl<$($name: Query),*> Query for ($($name,)*) {
            type Fetch = ($($name::Fetch,)*);
        }
    };
}

//smaller_tuples_too!(tuple_impl, B, A);
smaller_tuples_too!(tuple_impl, O, N, M, L, K, J, I, H, G, F, E, D, C, B, A);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_order() {
        assert!(Access::Write > Access::Read);
        assert!(Access::Read > Access::Iterate);
        assert!(Some(Access::Iterate) > None);
    }
}
