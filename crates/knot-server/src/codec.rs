//! Prost wire decoding with the service's explicit malformed-input status.
use prost::Message;
use std::marker::PhantomData;
use tonic::{
    Status,
    codec::{Codec as TonicCodec, DecodeBuf, Decoder},
};
pub struct Codec<T, U>(PhantomData<(T, U)>);
impl<T, U> Default for Codec<T, U> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
pub struct Input<U>(PhantomData<U>);
impl<U: Message + Default> Decoder for Input<U> {
    type Item = U;
    type Error = Status;
    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<U>, Status> {
        U::decode(buf)
            .map(Some)
            .map_err(|_| Status::invalid_argument("malformed protobuf"))
    }
}
impl<T: Message + Send + 'static, U: Message + Default + Send + 'static> TonicCodec
    for Codec<T, U>
{
    type Encode = T;
    type Decode = U;
    type Encoder = tonic_prost::ProstEncoder<T>;
    type Decoder = Input<U>;
    fn encoder(&mut self) -> Self::Encoder {
        tonic_prost::ProstEncoder::new(Default::default())
    }
    fn decoder(&mut self) -> Self::Decoder {
        Input(PhantomData)
    }
}
