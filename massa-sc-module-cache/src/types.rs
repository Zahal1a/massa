use massa_sc_runtime::RuntimeModule;
use massa_serialization::{
    Deserializer, SerializeError, Serializer, U64VarIntDeserializer, U64VarIntSerializer,
};
use nom::{
    error::{context, ContextError, ParseError},
    IResult, Parser,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::ops::Bound::Included;

/// Main type
#[derive(Clone)]
pub enum ModuleInfo {
    Invalid,
    Module(RuntimeModule),
    ModuleAndDelta((RuntimeModule, u64)),
}

/// Metadata type
pub enum ModuleMetadata {
    Absent,
    Invalid,
    Present(u64),
}

/// Metadata ID type
#[derive(IntoPrimitive, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u64)]
enum ModuleMetadataId {
    Absent = 0u64,
    Invalid = 1u64,
    Present = 2u64,
}

/// Metadata serializer
pub struct ModuleMetadataSerializer {
    u64_ser: U64VarIntSerializer,
}

impl ModuleMetadataSerializer {
    pub fn new() -> Self {
        Self {
            u64_ser: U64VarIntSerializer::new(),
        }
    }
}

impl Serializer<ModuleMetadata> for ModuleMetadataSerializer {
    fn serialize(
        &self,
        value: &ModuleMetadata,
        buffer: &mut Vec<u8>,
    ) -> Result<(), SerializeError> {
        match value {
            ModuleMetadata::Absent => self
                .u64_ser
                .serialize(&u64::from(ModuleMetadataId::Absent), buffer)?,
            ModuleMetadata::Invalid => self
                .u64_ser
                .serialize(&u64::from(ModuleMetadataId::Invalid), buffer)?,
            ModuleMetadata::Present(delta) => {
                self.u64_ser
                    .serialize(&u64::from(ModuleMetadataId::Present), buffer)?;
                self.u64_ser.serialize(delta, buffer)?;
            }
        }
        Ok(())
    }
}

/// Metadata deserializer
pub struct ModuleMetadataDeserializer {
    id_deser: U64VarIntDeserializer,
    delta_deser: U64VarIntDeserializer,
}

impl ModuleMetadataDeserializer {
    pub fn new() -> Self {
        Self {
            id_deser: U64VarIntDeserializer::new(
                Included(u64::from(ModuleMetadataId::Absent)),
                Included(u64::from(ModuleMetadataId::Present)),
            ),
            delta_deser: U64VarIntDeserializer::new(Included(0), Included(u64::MAX)),
        }
    }
}

impl Deserializer<ModuleMetadata> for ModuleMetadataDeserializer {
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], ModuleMetadata, E> {
        context("ModuleMetadata", |buffer| {
            // can unwrap here because the range is defined in the serializer setup
            let (input, id) = context("ModuleMetadataId", |input| self.id_deser.deserialize(input))
                .map(|id| ModuleMetadataId::try_from(id).unwrap())
                .parse(buffer)?;
            match id {
                ModuleMetadataId::Absent => Ok((input, ModuleMetadata::Absent)),
                ModuleMetadataId::Invalid => Ok((input, ModuleMetadata::Invalid)),
                ModuleMetadataId::Present => {
                    context("Delta", |input| self.delta_deser.deserialize(input))
                        .map(|delta| ModuleMetadata::Present(delta))
                        .parse(input)
                }
            }
        })
        .parse(buffer)
    }
}
