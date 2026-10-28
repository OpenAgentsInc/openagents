Rigid Data::skin(int body, uint32_t frame, int j) const {
  // MRC1's predecoded f32 rows, not the historical half texture format.
  // Decoding/normalization happened in pack_cast.py and is outside both timers.
  const auto *bytes=reinterpret_cast<const unsigned char*>(texels.data());
  float v[8];std::memcpy(v,bytes+(((size_t)body*framesPerBody+frame)*joints+j)*32,sizeof(v));
  return {{v[0],v[1],v[2],v[3]},{v[4],v[5],v[6]},false};
}
