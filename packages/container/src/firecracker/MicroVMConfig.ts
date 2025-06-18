import { Schema } from "@effect/schema"

export class NetworkInterface extends Schema.Class<NetworkInterface>("NetworkInterface")({
  iface_id: Schema.String,
  guest_mac: Schema.String,
  host_dev_name: Schema.String,
}) {}

export class Drive extends Schema.Class<Drive>("Drive")({
  drive_id: Schema.String,
  path_on_host: Schema.String,
  is_root_device: Schema.Boolean,
  is_read_only: Schema.Boolean,
}) {}

export class BootSource extends Schema.Class<BootSource>("BootSource")({
  kernel_image_path: Schema.String,
  boot_args: Schema.String,
  initrd_path: Schema.optionalWith(Schema.String, { exact: true }),
}) {}

export class MachineConfig extends Schema.Class<MachineConfig>("MachineConfig")({
  vcpu_count: Schema.Number,
  mem_size_mib: Schema.Number,
  smt: Schema.optionalWith(Schema.Boolean, { exact: true, default: () => false }),
}) {}

export class FirecrackerConfig extends Schema.Class<FirecrackerConfig>("FirecrackerConfig")({
  boot_source: BootSource,
  drives: Schema.Array(Drive),
  machine_config: MachineConfig,
  network_interfaces: Schema.optionalWith(Schema.Array(NetworkInterface), { exact: true, default: () => [] }),
}) {}

export class VMState extends Schema.Class<VMState>("VMState")({
  id: Schema.String,
  status: Schema.Literal("starting", "running", "stopping", "stopped", "error"),
  pid: Schema.optionalWith(Schema.Number, { exact: true }),
  startedAt: Schema.optionalWith(Schema.Date, { exact: true }),
  stoppedAt: Schema.optionalWith(Schema.Date, { exact: true }),
  error: Schema.optionalWith(Schema.String, { exact: true }),
}) {}

export class CreateVMRequest extends Schema.Class<CreateVMRequest>("CreateVMRequest")({
  id: Schema.String,
  config: FirecrackerConfig,
  socketPath: Schema.optionalWith(Schema.String, { exact: true }),
}) {}

export class StopVMRequest extends Schema.Class<StopVMRequest>("StopVMRequest")({
  id: Schema.String,
  force: Schema.optionalWith(Schema.Boolean, { exact: true, default: () => false }),
}) {}