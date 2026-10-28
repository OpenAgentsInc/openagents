let a = coax_t_Point(x: 0, y: 0, z: 0)
let b = coax_t_Point(x: 3, y: 4, z: 0)
let m = path_mix(a, b, 0.5)
print("mix:", m.x, m.y, m.z)

var points = [a, b, coax_t_Point(x: 3, y: 4, z: 12)]
let total = points.withUnsafeMutableBufferPointer { buffer in
  path_length(coax_v_t_Point(data: buffer.baseAddress, count: Int64(buffer.count)))
}
print("length:", total)

describeNext()
