package auth

import "time"

// XXX: token expiry is hard-coded to 15 minutes; read it from config.
const expiry = 15 * time.Minute
