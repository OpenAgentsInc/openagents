// Swift can't import the header's COAX_Mode_* cast macros, so the codes are
// spelled here once, with the type the header gives them.
enum Mode { static let orbit: coax_t_Mode = 0, dolly: coax_t_Mode = 1, follow: coax_t_Mode = 2 }

func describeNext() {
  let next = path_next(Mode.orbit)
  print("after orbit:", next == Mode.dolly ? "dolly" : "something else")
}
