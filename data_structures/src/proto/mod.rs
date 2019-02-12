use self::schema::witnet;
use crate::types::IpAddress;
use crate::{chain, types};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use failure::{ensure, format_err, Error};
use protobuf::Message;

pub mod schema;

/// Used for establishing correspondence between rust struct
/// and protobuf rust struct
pub trait ProtobufConvert: Sized {
    /// Type of the protobuf clone of Self
    type ProtoStruct;

    /// Struct -> ProtoStruct
    fn to_pb(&self) -> Self::ProtoStruct;

    /// ProtoStruct -> Struct
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error>;

    /// Struct -> ProtoStruct -> Bytes
    fn to_pb_bytes(&self) -> Result<Vec<u8>, Error>
    where
        Self::ProtoStruct: Message,
    {
        // Serialize
        self.to_pb().write_to_bytes().map_err(|e| e.into())
    }

    /// Bytes -> ProtoStruct -> Struct
    fn from_pb_bytes(bytes: &[u8]) -> Result<Self, Error>
    where
        Self::ProtoStruct: Message,
    {
        // Deserialize
        let mut a = Self::ProtoStruct::new();
        a.merge_from_bytes(bytes)?;
        Self::from_pb(a)
    }
}

impl ProtobufConvert for chain::RADType {
    type ProtoStruct = witnet::Transaction_Output_DataRequestOutput_RADRequest_RADType;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            chain::RADType::HttpGet => {
                witnet::Transaction_Output_DataRequestOutput_RADRequest_RADType::HttpGet
            }
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(match pb {
            witnet::Transaction_Output_DataRequestOutput_RADRequest_RADType::HttpGet => {
                chain::RADType::HttpGet
            }
        })
    }
}

// This will be hard to implement as a macro because one of the fields is an Option
impl ProtobufConvert for chain::LeadershipProof {
    type ProtoStruct = witnet::Block_LeadershipProof;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut m = witnet::Block_LeadershipProof::new();
        m.set_influence(self.influence);
        if let Some(sig) = &self.block_sig {
            m.set_block_sig(sig.to_pb());
        }
        m
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        let influence = pb.get_influence();
        let block_sig = match pb.optional_block_sig {
            Some(sig) => Some(chain::Signature::from_pb(match sig {
                witnet::Block_LeadershipProof_oneof_optional_block_sig::block_sig(s) => s,
            })?),
            None => None,
        };
        Ok(chain::LeadershipProof {
            block_sig,
            influence,
        })
    }
}

impl ProtobufConvert for chain::Signature {
    type ProtoStruct = witnet::Signature;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut m = witnet::Signature::new();
        match self {
            chain::Signature::Secp256k1(chain::Secp256k1Signature { r, s, v }) => {
                let mut x = witnet::Secp256k1Signature::new();
                let mut sv = vec![];
                sv.extend(s);
                sv.extend(&[*v]);
                x.set_r(r.to_vec());
                x.set_s(sv);
                m.set_s(x);
            }
        }
        m
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        match pb.kind {
            Some(witnet::Signature_oneof_kind::s(mut x)) => {
                let vr = x.take_r();
                let vsv = x.take_s();

                if vr.len() == 32 && vsv.len() == 33 {
                    let mut r = [0; 32];
                    let mut s = [0; 32];
                    let v = vsv[32];
                    r.copy_from_slice(&vr);
                    s.copy_from_slice(&vsv[..32]);
                    Ok(chain::Signature::Secp256k1(chain::Secp256k1Signature {
                        r,
                        s,
                        v,
                    }))
                } else {
                    Err(format_err!("Invalid signature byte length"))
                }
            }
            None => Err(format_err!("Invalid signature type")),
        }
    }
}

impl ProtobufConvert for types::Address {
    type ProtoStruct = witnet::Address;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut address = witnet::Address::new();
        let mut bytes = vec![];
        match self.ip {
            IpAddress::Ipv4 { ip } => {
                bytes.write_u32::<BigEndian>(ip).unwrap();
            }
            IpAddress::Ipv6 { ip0, ip1, ip2, ip3 } => {
                bytes.write_u32::<BigEndian>(ip0).unwrap();
                bytes.write_u32::<BigEndian>(ip1).unwrap();
                bytes.write_u32::<BigEndian>(ip2).unwrap();
                bytes.write_u32::<BigEndian>(ip3).unwrap();
            }
        }
        bytes.write_u16::<BigEndian>(self.port).unwrap();
        address.set_address(bytes);

        address
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        let mut bytes = pb.get_address();
        match bytes.len() {
            6 => {
                // Ipv4
                let ip = bytes.read_u32::<BigEndian>().unwrap();
                let ip = types::IpAddress::Ipv4 { ip };
                let port = bytes.read_u16::<BigEndian>().unwrap();

                Ok(types::Address { ip, port })
            }
            18 => {
                // Ipv6
                let ip0 = bytes.read_u32::<BigEndian>().unwrap();
                let ip1 = bytes.read_u32::<BigEndian>().unwrap();
                let ip2 = bytes.read_u32::<BigEndian>().unwrap();
                let ip3 = bytes.read_u32::<BigEndian>().unwrap();
                let port = bytes.read_u16::<BigEndian>().unwrap();
                let ip = types::IpAddress::Ipv6 { ip0, ip1, ip2, ip3 };

                Ok(types::Address { ip, port })
            }
            _ => Err(format_err!("Invalid address size")),
        }
    }
}

impl ProtobufConvert for String {
    type ProtoStruct = Self;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.clone()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(pb)
    }
}

impl<T> ProtobufConvert for Vec<T>
where
    T: ProtobufConvert,
{
    type ProtoStruct = Vec<T::ProtoStruct>;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.iter().map(|v| v.to_pb()).collect()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        pb.into_iter()
            .map(ProtobufConvert::from_pb)
            .collect::<Result<Vec<_>, _>>()
    }
}

impl ProtobufConvert for Vec<u8> {
    type ProtoStruct = Self;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.clone()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(pb)
    }
}

impl ProtobufConvert for [u8; 20] {
    type ProtoStruct = Vec<u8>;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.to_vec()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(pb.len() == 20, "Invalid array length");
        let mut x = [0; 20];
        x.copy_from_slice(&pb);
        Ok(x)
    }
}

impl ProtobufConvert for [u8; 32] {
    type ProtoStruct = Vec<u8>;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.to_vec()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(pb.len() == 32, "Invalid array length");
        let mut x = [0; 32];
        x.copy_from_slice(&pb);
        Ok(x)
    }
}

macro_rules! impl_protobuf_convert_scalar {
    ($name:tt) => {
        impl ProtobufConvert for $name {
            type ProtoStruct = $name;
            fn to_pb(&self) -> Self::ProtoStruct {
                *self
            }
            fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
                Ok(pb)
            }
        }
    };
}

impl_protobuf_convert_scalar!(bool);
impl_protobuf_convert_scalar!(u32);
impl_protobuf_convert_scalar!(u64);
impl_protobuf_convert_scalar!(i32);
impl_protobuf_convert_scalar!(i64);
impl_protobuf_convert_scalar!(f32);
impl_protobuf_convert_scalar!(f64);

// Conflicts with Vec<u8>
/*
impl ProtobufConvert for u8 {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        Self::ProtoStruct::from(*self)
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            pb <= Self::ProtoStruct::from(Self::max_value()),
            "Integer out of range"
        );
        Ok(pb as Self)
    }
}
*/

impl ProtobufConvert for i8 {
    type ProtoStruct = i32;
    fn to_pb(&self) -> Self::ProtoStruct {
        Self::ProtoStruct::from(*self)
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            pb <= Self::ProtoStruct::from(Self::max_value()),
            "Integer out of range"
        );
        Ok(pb as Self)
    }
}

impl ProtobufConvert for u16 {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        Self::ProtoStruct::from(*self)
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            pb <= Self::ProtoStruct::from(Self::max_value()),
            "Integer out of range"
        );
        Ok(pb as Self)
    }
}

impl ProtobufConvert for i16 {
    type ProtoStruct = i32;
    fn to_pb(&self) -> Self::ProtoStruct {
        Self::ProtoStruct::from(*self)
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            pb <= Self::ProtoStruct::from(Self::max_value()),
            "Integer out of range"
        );
        Ok(pb as Self)
    }
}