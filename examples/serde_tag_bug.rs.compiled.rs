#![feature(prelude_import)]
#![no_std]
#[prelude_import]
use std::prelude::v1::*;
#[macro_use]
extern crate std as std;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

#[serde(tag = "tag")]
pub enum A {
    A1 { field: B },
    A2,
}
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _IMPL_SERIALIZE_FOR_A: () = {
    extern crate serde as _serde;
    #[automatically_derived]
    impl _serde::Serialize for A {
        fn serialize<__S>(&self, _serializer: __S) -> _serde::export::Result<__S::Ok, __S::Error>
            where __S: _serde::Serializer
        {
            match *self {
                A::A1 { ref field } => {
                    let mut __serde_state =

                            // Construct an A containing a B.

                            // Successfully serialises to `{"tag":"a","field":"B1"}`.

                            // That correct, serde_json-generated JSON errors in the decode with
                            // `Syntax(Message("invalid type: string \"B1\", expected enum B"), 0, 0)`
                            match _serde::Serializer::serialize_struct(_serializer,
                                                                       "A",
                                                                       0 + 1 +
                                                                           1)
                                {
                                ::result::Result::Ok(val) => val,
                                ::result::Result::Err(err) => {
                                    return ::result::Result::Err(::convert::From::from(err))
                                }
                            };
                    match _serde::ser::SerializeStruct::serialize_field(&mut __serde_state,
                                                                        "tag",
                                                                        "A1") {
                        ::result::Result::Ok(val) => val,
                        ::result::Result::Err(err) => {
                            return ::result::Result::Err(::convert::From::from(err))
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(&mut __serde_state,
                                                                        "field",
                                                                        field) {
                        ::result::Result::Ok(val) => val,
                        ::result::Result::Err(err) => {
                            return ::result::Result::Err(::convert::From::from(err))
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
                A::A2 => {
                    let mut __struct =
                        match _serde::Serializer::serialize_struct(_serializer, "A", 1) {
                            ::result::Result::Ok(val) => val,
                            ::result::Result::Err(err) => {
                                return ::result::Result::Err(::convert::From::from(err))
                            }
                        };
                    match _serde::ser::SerializeStruct::serialize_field(&mut __struct,
                                                                        "tag",
                                                                        "A2") {
                        ::result::Result::Ok(val) => val,
                        ::result::Result::Err(err) => {
                            return ::result::Result::Err(::convert::From::from(err))
                        }
                    };
                    _serde::ser::SerializeStruct::end(__struct)
                }
            }
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _IMPL_DESERIALIZE_FOR_A: () = {
    extern crate serde as _serde;
    #[automatically_derived]
    impl _serde::Deserialize for A {
        fn deserialize<__D>(deserializer: __D) -> _serde::export::Result<A, __D::Error>
            where __D: _serde::Deserializer
        {
            #[allow(non_camel_case_types)]
            enum __Field {
                __field0,
                __field1,
            }
            impl _serde::Deserialize for __Field {
                #[inline]
                fn deserialize<__D>(deserializer: __D)
                                    -> _serde::export::Result<__Field, __D::Error>
                    where __D: _serde::Deserializer
                {
                    struct __FieldVisitor;
                    impl _serde::de::Visitor for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(&self,
                                     formatter: &mut _serde::export::fmt::Formatter)
                                     -> _serde::export::fmt::Result {
                            _serde::export::fmt::Formatter::write_str(formatter, "field name")
                        }
                        fn visit_u32<__E>(self, value: u32) -> _serde::export::Result<__Field, __E>
                            where __E: _serde::de::Error
                        {
                            match value {
                                0u32 => _serde::export::Ok(__Field::__field0),
                                1u32 => _serde::export::Ok(__Field::__field1),
                                _ =>
                                    _serde::export::Err(_serde::de::Error::invalid_value(_serde::de::Unexpected::Unsigned(value
                                                                                                                              as
                                                                                                                              u64),
                                                                                         &"variant index 0 <= i < 2")),
                            }
                        }
                        fn visit_str<__E>(self, value: &str) -> _serde::export::Result<__Field, __E>
                            where __E: _serde::de::Error
                        {
                            match value {
                                "A1" => _serde::export::Ok(__Field::__field0),
                                "A2" => _serde::export::Ok(__Field::__field1),
                                _ =>
                                    _serde::export::Err(_serde::de::Error::unknown_variant(value,
                                                                                           VARIANTS)),
                            }
                        }
                        fn visit_bytes<__E>(self,
                                            value: &[u8])
                                            -> _serde::export::Result<__Field, __E>
                            where __E: _serde::de::Error
                        {
                            match value {
                                b"A1" => _serde::export::Ok(__Field::__field0),
                                b"A2" => _serde::export::Ok(__Field::__field1),
                                _ => {
                                    let value = &_serde::export::from_utf8_lossy(value);
                                    _serde::export::Err(_serde::de::Error::unknown_variant(value,
                                                                                               VARIANTS))
                                }
                            }
                        }
                    }
                    _serde::Deserializer::deserialize_struct_field(deserializer, __FieldVisitor)
                }
            }
            const VARIANTS: &'static [&'static str] = &["A1", "A2"];
            let _tagged =
                    match _serde::Deserializer::deserialize(deserializer,
                                                            _serde::de::private::TaggedContentVisitor::<__Field>::new("tag"))
                        {
                        ::result::Result::Ok(val) => val,
                        ::result::Result::Err(err) => {
                            return ::result::Result::Err(::convert::From::from(err))
                        }
                    };
            match _tagged.tag {
                __Field::__field0 => {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __ignore,
                    }
                    impl _serde::Deserialize for __Field {
                        #[inline]
                        fn deserialize<__D>(deserializer: __D)
                                            -> _serde::export::Result<__Field, __D::Error>
                            where __D: _serde::Deserializer
                        {
                            struct __FieldVisitor;
                            impl _serde::de::Visitor for __FieldVisitor {
                                type Value = __Field;
                                fn expecting(&self,
                                             formatter: &mut _serde::export::fmt::Formatter)
                                             -> _serde::export::fmt::Result {
                                    _serde::export::fmt::Formatter::write_str(formatter,
                                                                              "field name")
                                }
                                fn visit_str<__E>(self,
                                                  value: &str)
                                                  -> _serde::export::Result<__Field, __E>
                                    where __E: _serde::de::Error
                                {
                                    match value {
                                        "field" => _serde::export::Ok(__Field::__field0),
                                        _ => _serde::export::Ok(__Field::__ignore),
                                    }
                                }
                                fn visit_bytes<__E>(self,
                                                    value: &[u8])
                                                    -> _serde::export::Result<__Field, __E>
                                    where __E: _serde::de::Error
                                {
                                    match value {
                                        b"field" => _serde::export::Ok(__Field::__field0),
                                        _ => _serde::export::Ok(__Field::__ignore),
                                    }
                                }
                            }
                            _serde::Deserializer::deserialize_struct_field(deserializer,
                                                                           __FieldVisitor)
                        }
                    }
                    struct __Visitor;
                    impl _serde::de::Visitor for __Visitor {
                        type Value = A;
                        fn expecting(&self,
                                     formatter: &mut _serde::export::fmt::Formatter)
                                     -> _serde::export::fmt::Result {
                            _serde::export::fmt::Formatter::write_str(formatter,
                                                                      "struct variant A::A1")
                        }
                        #[inline]
                        fn visit_map<__V>(self,
                                          mut visitor: __V)
                                          -> _serde::export::Result<A, __V::Error>
                            where __V: _serde::de::MapVisitor
                        {
                            let mut __field0: _serde::export::Option<B> = _serde::export::None;
                            while let _serde::export::Some(key) =
                                match _serde::de::MapVisitor::visit_key::<__Field>(&mut visitor) {
                                    ::result::Result::Ok(val) => val,
                                    ::result::Result::Err(err) => {
                                        return ::result::Result::Err(::convert::From::from(err))
                                    }
                                } {
                                match key {
                                    __Field::__field0 => {
                                        if _serde::export::Option::is_some(&__field0) {
                                            return _serde::export::Err(<__V::Error
                                                                               as
                                                                               _serde::de::Error>::duplicate_field("field"));
                                        }
                                        __field0 =
                                                _serde::export::Some(match _serde::de::MapVisitor::visit_value::<B>(&mut visitor)
                                                                         {
                                                                         ::result::Result::Ok(val)
                                                                         =>
                                                                         val,
                                                                         ::result::Result::Err(err)
                                                                         => {
                                                                             return ::result::Result::Err(::convert::From::from(err))
                                                                         }
                                                                     });
                                    }
                                    _ => {
                                        let _ =
                                                match _serde::de::MapVisitor::visit_value::<_serde::de::impls::IgnoredAny>(&mut visitor)
                                                    {
                                                    ::result::Result::Ok(val)
                                                    => val,
                                                    ::result::Result::Err(err)
                                                    => {
                                                        return ::result::Result::Err(::convert::From::from(err))
                                                    }
                                                };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::export::Some(__field0) => __field0,
                                _serde::export::None => {
                                    match _serde::de::private::missing_field("field") {
                                        ::result::Result::Ok(val) => val,
                                        ::result::Result::Err(err) => {
                                            return ::result::Result::Err(::convert::From::from(err))
                                        }
                                    }
                                }
                            };
                            _serde::export::Ok(A::A1 { field: __field0 })
                        }
                    }
                    const FIELDS: &'static [&'static str] = &["field"];
                    _serde::Deserializer::deserialize(_serde::de::private::ContentDeserializer::<__D::Error>::new(_tagged.content),
                                                          __Visitor)
                }
                __Field::__field1 => {
                    match _serde::Deserializer::deserialize(_serde::de::private::ContentDeserializer::<__D::Error>::new(_tagged.content),
                                                                _serde::de::private::InternallyTaggedUnitVisitor::new("A",
                                                                                                                      "A2"))
                            {
                            ::result::Result::Ok(val) => val,
                            ::result::Result::Err(err) => {
                                return ::result::Result::Err(::convert::From::from(err))
                            }
                        };
                    _serde::export::Ok(A::A2)
                }
            }
        }
    }
};
pub enum B {
    B1,
    B2,
}
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _IMPL_SERIALIZE_FOR_B: () = {
    extern crate serde as _serde;
    #[automatically_derived]
    impl _serde::Serialize for B {
        fn serialize<__S>(&self, _serializer: __S) -> _serde::export::Result<__S::Ok, __S::Error>
            where __S: _serde::Serializer
        {
            match *self {
                B::B1 => _serde::Serializer::serialize_unit_variant(_serializer, "B", 0usize, "B1"),
                B::B2 => _serde::Serializer::serialize_unit_variant(_serializer, "B", 1usize, "B2"),
            }
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _IMPL_DESERIALIZE_FOR_B: () = {
    extern crate serde as _serde;
    #[automatically_derived]
    impl _serde::Deserialize for B {
        fn deserialize<__D>(deserializer: __D) -> _serde::export::Result<B, __D::Error>
            where __D: _serde::Deserializer
        {
            #[allow(non_camel_case_types)]
            enum __Field {
                __field0,
                __field1,
            }
            impl _serde::Deserialize for __Field {
                #[inline]
                fn deserialize<__D>(deserializer: __D)
                                    -> _serde::export::Result<__Field, __D::Error>
                    where __D: _serde::Deserializer
                {
                    struct __FieldVisitor;
                    impl _serde::de::Visitor for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(&self,
                                     formatter: &mut _serde::export::fmt::Formatter)
                                     -> _serde::export::fmt::Result {
                            _serde::export::fmt::Formatter::write_str(formatter, "field name")
                        }
                        fn visit_u32<__E>(self, value: u32) -> _serde::export::Result<__Field, __E>
                            where __E: _serde::de::Error
                        {
                            match value {
                                0u32 => _serde::export::Ok(__Field::__field0),
                                1u32 => _serde::export::Ok(__Field::__field1),
                                _ =>
                                    _serde::export::Err(_serde::de::Error::invalid_value(_serde::de::Unexpected::Unsigned(value
                                                                                                                              as
                                                                                                                              u64),
                                                                                         &"variant index 0 <= i < 2")),
                            }
                        }
                        fn visit_str<__E>(self, value: &str) -> _serde::export::Result<__Field, __E>
                            where __E: _serde::de::Error
                        {
                            match value {
                                "B1" => _serde::export::Ok(__Field::__field0),
                                "B2" => _serde::export::Ok(__Field::__field1),
                                _ =>
                                    _serde::export::Err(_serde::de::Error::unknown_variant(value,
                                                                                           VARIANTS)),
                            }
                        }
                        fn visit_bytes<__E>(self,
                                            value: &[u8])
                                            -> _serde::export::Result<__Field, __E>
                            where __E: _serde::de::Error
                        {
                            match value {
                                b"B1" => _serde::export::Ok(__Field::__field0),
                                b"B2" => _serde::export::Ok(__Field::__field1),
                                _ => {
                                    let value = &_serde::export::from_utf8_lossy(value);
                                    _serde::export::Err(_serde::de::Error::unknown_variant(value,
                                                                                               VARIANTS))
                                }
                            }
                        }
                    }
                    _serde::Deserializer::deserialize_struct_field(deserializer, __FieldVisitor)
                }
            }
            struct __Visitor;
            impl _serde::de::Visitor for __Visitor {
                type Value = B;
                fn expecting(&self,
                             formatter: &mut _serde::export::fmt::Formatter)
                             -> _serde::export::fmt::Result {
                    _serde::export::fmt::Formatter::write_str(formatter, "enum B")
                }
                fn visit_enum<__V>(self, visitor: __V) -> _serde::export::Result<B, __V::Error>
                    where __V: _serde::de::EnumVisitor
                {
                    match match _serde::de::EnumVisitor::visit_variant(visitor) {
                        ::result::Result::Ok(val) => val,
                        ::result::Result::Err(err) => {
                            return ::result::Result::Err(::convert::From::from(err))
                        }
                    } {
                        (__Field::__field0, visitor) => {
                            match _serde::de::VariantVisitor::visit_unit(visitor) {
                                ::result::Result::Ok(val) => val,
                                ::result::Result::Err(err) => {
                                    return ::result::Result::Err(::convert::From::from(err))
                                }
                            };
                            _serde::export::Ok(B::B1)
                        }
                        (__Field::__field1, visitor) => {
                            match _serde::de::VariantVisitor::visit_unit(visitor) {
                                ::result::Result::Ok(val) => val,
                                ::result::Result::Err(err) => {
                                    return ::result::Result::Err(::convert::From::from(err))
                                }
                            };
                            _serde::export::Ok(B::B2)
                        }
                    }
                }
            }
            const VARIANTS: &'static [&'static str] = &["B1", "B2"];
            _serde::Deserializer::deserialize_enum(deserializer, "B", VARIANTS, __Visitor)
        }
    }
};
fn main() {
    let a = A::A1 { field: B::B1 };
    let json = serde_json::to_string(&a).unwrap();
    ::io::_print(::std::fmt::Arguments::new_v1({
                                                   static __STATIC_FMTSTR:
                                                          &'static [&'static str]
                                                          =
                                                       &["", "\n"];
                                                   __STATIC_FMTSTR
                                               },
                                               &match (&json,) {
                                                   (__arg0,) =>
                                                    [::std::fmt::ArgumentV1::new(__arg0,
                                                                                 ::std::fmt::Display::fmt)],
                                               }));
    let a_new: A = serde_json::from_str(json.as_str()).unwrap();
    ::io::_print(::std::fmt::Arguments::new_v1({
                                                   static __STATIC_FMTSTR:
                                                          &'static [&'static str]
                                                          =
                                                       &["", "\n"];
                                                   __STATIC_FMTSTR
                                               },
                                               &match (&a_new,) {
                                                   (__arg0,) =>
                                                    [::std::fmt::ArgumentV1::new(__arg0,
                                                                                 ::std::fmt::Debug::fmt)],
                                               }));
}
