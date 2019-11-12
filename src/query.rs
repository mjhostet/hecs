use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::archetype::Archetype;
use crate::world::EntityMeta;
use crate::{Component, Entity};

/// A collection of component types to fetch from a `World`
pub trait Query<'a> {
    #[doc(hidden)]
    type Fetch: Fetch<'a>;
}

#[doc(hidden)]
pub trait Fetch<'a>: Sized {
    type Item;
    fn get(archetype: &Archetype) -> Option<Self>;
    unsafe fn next(&mut self) -> Self::Item;
}

impl<'a, T: Component> Query<'a> for &'a T {
    type Fetch = FetchRead<T>;
}

#[doc(hidden)]
pub struct FetchRead<T>(NonNull<T>);

impl<'a, T: Component> Fetch<'a> for FetchRead<T> {
    type Item = &'a T;
    fn get(archetype: &Archetype) -> Option<Self> {
        archetype.data::<T>().map(Self)
    }
    unsafe fn next(&mut self) -> &'a T {
        let x = self.0.as_ptr();
        self.0 = NonNull::new_unchecked(x.add(1));
        &*x
    }
}

impl<'a, T: Component> Query<'a> for Option<&'a T> {
    type Fetch = FetchTryRead<T>;
}

#[doc(hidden)]
pub struct FetchTryRead<T>(Option<NonNull<T>>);

impl<'a, T: Component> Fetch<'a> for FetchTryRead<T> {
    type Item = Option<&'a T>;
    fn get(archetype: &Archetype) -> Option<Self> {
        Some(Self(archetype.data::<T>()))
    }
    unsafe fn next(&mut self) -> Option<&'a T> {
        let x = self.0?.as_ptr();
        self.0 = Some(NonNull::new_unchecked(x.add(1)));
        Some(&*x)
    }
}

impl<'a, T: Component> Query<'a> for &'a mut T {
    type Fetch = FetchWrite<T>;
}

#[doc(hidden)]
pub struct FetchWrite<T>(NonNull<T>);

impl<'a, T: Component> Fetch<'a> for FetchWrite<T> {
    type Item = &'a mut T;
    fn get(archetype: &Archetype) -> Option<Self> {
        archetype.data::<T>().map(Self)
    }
    unsafe fn next(&mut self) -> &'a mut T {
        let x = self.0.as_ptr();
        self.0 = NonNull::new_unchecked(x.add(1));
        &mut *x
    }
}

impl<'a, T: Component> Query<'a> for Option<&'a mut T> {
    type Fetch = FetchTryWrite<T>;
}

#[doc(hidden)]
pub struct FetchTryWrite<T>(Option<NonNull<T>>);

impl<'a, T: Component> Fetch<'a> for FetchTryWrite<T> {
    type Item = Option<&'a mut T>;
    fn get(archetype: &Archetype) -> Option<Self> {
        Some(Self(archetype.data::<T>()))
    }
    unsafe fn next(&mut self) -> Option<&'a mut T> {
        let x = self.0?.as_ptr();
        self.0 = Some(NonNull::new_unchecked(x.add(1)));
        Some(&mut *x)
    }
}

/// Iterator over the set of entities with the components required by `Q`
pub struct QueryIter<'a, Q: Query<'a>> {
    meta: &'a [EntityMeta],
    archetypes: std::slice::IterMut<'a, Archetype>,
    iter: Option<ChunkIter<'a, Q::Fetch>>,
}

impl<'a, Q: Query<'a>> QueryIter<'a, Q> {
    pub(crate) fn new(meta: &'a [EntityMeta], archetypes: &'a mut [Archetype]) -> Self {
        Self {
            meta,
            archetypes: archetypes.iter_mut(),
            iter: None,
        }
    }
}

impl<'a, Q: Query<'a>> Iterator for QueryIter<'a, Q> {
    type Item = (Entity, <<Q as Query<'a>>::Fetch as Fetch<'a>>::Item);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter {
                None => {
                    let archetype = self.archetypes.next()?;
                    self.iter = Q::Fetch::get(archetype).map(|fetch| ChunkIter {
                        entities: archetype.entities(),
                        fetch,
                        len: archetype.len(),
                        _marker: PhantomData,
                    });
                }
                Some(ref mut iter) => match iter.next() {
                    None => {
                        self.iter = None;
                    }
                    Some((id, item)) => {
                        return Some((
                            Entity {
                                id,
                                generation: self.meta[id as usize].generation,
                            },
                            item,
                        ));
                    }
                },
            }
        }
    }
}

struct ChunkIter<'a, T: Fetch<'a>> {
    entities: NonNull<u32>,
    fetch: T,
    len: usize,
    _marker: PhantomData<&'a ()>,
}

impl<'a, T: Fetch<'a>> Iterator for ChunkIter<'a, T> {
    type Item = (u32, T::Item);
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let entity = self.entities.as_ptr();
        unsafe {
            self.entities = NonNull::new_unchecked(entity.add(1));
            Some((*entity, self.fetch.next()))
        }
    }
}

macro_rules! tuple_impl {
    ($($name: ident),*) => {
        impl<'a, $($name: Fetch<'a>),*> Fetch<'a> for ($($name,)*) {
            type Item = ($($name::Item,)*);
            #[allow(unused_variables)]
            fn get(archetype: &Archetype) -> Option<Self> {
                Some(($($name::get(archetype)?,)*))
            }
            unsafe fn next(&mut self) -> Self::Item {
                #[allow(non_snake_case)]
                let ($($name,)*) = self;
                ($($name.next(),)*)
            }
        }

        impl<'a, $($name: Query<'a>),*> Query<'a> for ($($name,)*) {
            type Fetch = (($($name::Fetch,)*));
        }
    }
}

tuple_impl!();
tuple_impl!(A);
tuple_impl!(A, B);
tuple_impl!(A, B, C);
tuple_impl!(A, B, C, D);
tuple_impl!(A, B, C, D, E);
tuple_impl!(A, B, C, D, E, F);
tuple_impl!(A, B, C, D, E, F, G);
tuple_impl!(A, B, C, D, E, F, G, H);
tuple_impl!(A, B, C, D, E, F, G, H, I);
tuple_impl!(A, B, C, D, E, F, G, H, I, J);
tuple_impl!(A, B, C, D, E, F, G, H, I, J, K);
tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L);
tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M);
tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF);
// tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG);
