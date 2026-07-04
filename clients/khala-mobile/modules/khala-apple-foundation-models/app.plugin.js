const { withInfoPlist } = require("@expo/config-plugins")

module.exports = function withKhalaAppleFoundationModels(config) {
  return withInfoPlist(config, mod => {
    mod.modResults.KhalaAppleFoundationModelsBridge = {
      status: "local_bridge_required",
      reference: "clients/khala-ios/Khala/Khala/Net/AppleFMClient.swift"
    }
    return mod
  })
}
