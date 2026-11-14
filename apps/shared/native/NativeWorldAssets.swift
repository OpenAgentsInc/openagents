// Firm reason: 6 (URLSession, CryptoKit and filesystem effects for Coax-admitted assets).
import Foundation
import CryptoKit

struct NativeWorldFile {
    let path:String,hash:String
    let bytes:Int64,required:Bool
}
struct NativeWorldInventory {
    let realm:String,runtime:String,identity:String
    let files:[NativeWorldFile]
    @MainActor init(_ raw:Data)throws {
        let manifest=UnsafeMutablePointer<coax_t_NwManifest>.allocate(capacity:1)
        manifest.initialize(to:coax_t_NwManifest());defer {manifest.deinitialize(count:1);manifest.deallocate()}
        let view=coax_v_t_NwManifest(data:manifest,count:1)
        var ok=false
        NativeWorldHost.deep.run {ok=raw.withUnsafeBytes {nw_read(coax_string(data:$0.baseAddress?.assumingMemoryBound(to:CChar.self),count:Int64($0.count)),view)}}
        guard ok else {throw NativeFailure.refused("The world's download inventory is invalid.")}
        func text(_ row:Int64,_ field:Int64)->String {
            var out=[UInt8](repeating:0,count:128)
            let n=out.withUnsafeMutableBufferPointer {nw_text(view,row,field,coax_v_u8(data:$0.baseAddress,count:Int64($0.count)))}
            return n<0 ? "":String(decoding:out.prefix(Int(n)),as:UTF8.self)
        }
        realm=text(-1,0);runtime=text(-1,1);identity=NativePackage.hex(Data(SHA256.hash(data:raw)))
        files=(0..<manifest.pointee.n).map {NativeWorldFile(path:text($0,0),hash:text($0,1),bytes:nw_bytes(view,$0),required:nw_required(view,$0))}
    }
}

// Neither public objects nor API calls follow redirects with account headers.
final class NativeWorldSession:NSObject,URLSessionTaskDelegate,@unchecked Sendable {
    private let configuration:URLSessionConfiguration?
    init(configuration:URLSessionConfiguration?=nil) {self.configuration=configuration;super.init()}
    lazy var session:URLSession = {
        let c=configuration ?? URLSessionConfiguration.ephemeral;c.httpShouldSetCookies=false;c.urlCache=nil;c.timeoutIntervalForRequest=60;c.timeoutIntervalForResource=180
        return URLSession(configuration:c,delegate:self,delegateQueue:nil)
    }()
    func urlSession(_ session:URLSession,task:URLSessionTask,willPerformHTTPRedirection response:HTTPURLResponse,newRequest request:URLRequest,completionHandler:@escaping(URLRequest?)->Void) {completionHandler(nil)}
    func close() {session.invalidateAndCancel()}
}

actor NativeWorldAssets {
    let home:URL
    private let transport:NativeWorldSession
    init(home:URL?=nil,transport:NativeWorldSession=NativeWorldSession()) {
        self.transport=transport
        self.home=home ?? URL(fileURLWithPath:NSHomeDirectory(),isDirectory:true).appendingPathComponent(".openagents/worlds",isDirectory:true)
    }
    static func hash(_ file:URL)throws->(Int64,String) {
        let h=try FileHandle(forReadingFrom:file);defer {try? h.close()};var digest=SHA256(),count:Int64=0
        while let data=try h.read(upToCount:65536),!data.isEmpty {count+=Int64(data.count);digest.update(data:data)}
        return (count,NativePackage.hex(Data(digest.finalize())))
    }
    func directory(_ inventory:NativeWorldInventory)->URL {home.appendingPathComponent("installed/\(inventory.realm)/\(inventory.identity)",isDirectory:true)}
    private func valid(_ url:URL,_ f:NativeWorldFile)->Bool {
        guard (try? url.resourceValues(forKeys:[.isSymbolicLinkKey]).isSymbolicLink) != true,
              let (count,hash)=try? Self.hash(url) else {return false}
        return f.hash.withCString {expected in hash.withCString {actual in nwp_received(f.bytes,count,coax_string(data:expected,count:64),coax_string(data:actual,count:64),false)}}
    }
    func prepare(_ inventory:NativeWorldInventory,progress:@escaping @Sendable(Int64,Int64)->Void)async throws->URL {
        let needed=inventory.files.filter(\.required),total=needed.reduce(Int64(0)) {$0+$1.bytes};var done:Int64=0
        let root=directory(inventory)
        for file in needed {let completed=done;try await install(file,root:root) {n in progress(completed+n,total)};done+=file.bytes;progress(done,total)}
        return root
    }
    func optional(_ inventory:NativeWorldInventory,installed:@escaping @Sendable(String)->Void)async {
        let root=directory(inventory)
        for file in inventory.files where !file.required && !["jpg","jpeg","png","webp"].contains((file.path as NSString).pathExtension.lowercased()) {
            if Task.isCancelled {return}
            do {try await install(file,root:root,progress:{_ in});installed(file.path)}catch {if Task.isCancelled {return}}
        }
    }
    func fetch(_ path:String,inventory:NativeWorldInventory)async throws {
        guard let file=inventory.files.first(where:{$0.path==path}) else {throw NativeFailure.refused("World asset was not named by the signed inventory.")}
        try await install(file,root:directory(inventory),progress:{_ in})
    }
    private func install(_ file:NativeWorldFile,root:URL,progress:@escaping @Sendable(Int64)->Void)async throws {
        try Task.checkCancellation()
        let fm=FileManager.default,destination=root.appendingPathComponent(file.path),object=home.appendingPathComponent("objects/\(file.hash)")
        if valid(destination,file) {progress(file.bytes);return}
        try fm.createDirectory(at:object.deletingLastPathComponent(),withIntermediateDirectories:true,attributes:[.posixPermissions:0o700])
        if !valid(object,file) {
            let temporary=home.appendingPathComponent("objects/.\(UUID().uuidString).partial")
            defer {try? fm.removeItem(at:temporary)}
            guard fm.createFile(atPath:temporary.path,contents:nil,attributes:[.posixPermissions:0o600]) else {throw NativeFailure.refused("Not enough storage for this world.")}
            let handle=try FileHandle(forWritingTo:temporary);defer {try? handle.close()}
            let url=URL(string:"https://storage.googleapis.com/openagentsgemini-native-world-assets/objects/\(file.hash)")!
            let (stream,response)=try await transport.session.bytes(for:URLRequest(url:url))
            guard let http=response as? HTTPURLResponse,http.statusCode==200,
                  response.expectedContentLength<0 || response.expectedContentLength==file.bytes else {throw NativeFailure.refused("A world file could not be downloaded. Try again.")}
            var count:Int64=0,digest=SHA256(),chunk=Data();chunk.reserveCapacity(65536)
            for try await byte in stream {
                guard count<file.bytes else {throw NativeFailure.refused("A world file exceeded its declared size.")}
                chunk.append(byte);count+=1
                if chunk.count==65536 {try Task.checkCancellation();try handle.write(contentsOf:chunk);digest.update(data:chunk);chunk.removeAll(keepingCapacity:true);progress(count)}
            }
            try Task.checkCancellation();if !chunk.isEmpty {try handle.write(contentsOf:chunk);digest.update(data:chunk)}
            let actual=NativePackage.hex(Data(digest.finalize()))
            let admitted=file.hash.withCString {expected in actual.withCString {observed in nwp_received(file.bytes,count,coax_string(data:expected,count:64),coax_string(data:observed,count:64),Task.isCancelled)}}
            guard admitted else {throw NativeFailure.refused("A world file failed its integrity check. Try again.")}
            try handle.synchronize();try handle.close()
            if fm.fileExists(atPath:object.path) {try fm.removeItem(at:object)}
            try fm.moveItem(at:temporary,to:object)
        }
        try Task.checkCancellation()
        try fm.createDirectory(at:destination.deletingLastPathComponent(),withIntermediateDirectories:true,attributes:[.posixPermissions:0o700])
        if fm.fileExists(atPath:destination.path) {try fm.removeItem(at:destination)}
        try fm.linkItem(at:object,to:destination)
        progress(file.bytes)
    }
}
