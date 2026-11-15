// Firm reason: 6 (UIKit document consent and Foundation bytes/credential-free upload effects).
import UIKit
import UniformTypeIdentifiers

@MainActor final class NativeDocuments:NSObject,UIDocumentPickerDelegate {
    private var completion:((Result<[String:Any],Error>)->Void)?
    private var limit:Int64=67108864
    private var held:[String:Data]=[:]
    private let upload=NativeWorldSession()
    func pick(_ presenter:UIViewController,types:[UTType] = [.data],limit:Int64=67108864)async throws->[String:Any] {
        guard completion==nil,presenter.presentedViewController==nil else {throw NativeFailure.refused("Close the current panel before choosing a file.")}
        self.limit=limit
        return try await withCheckedThrowingContinuation { done in
            completion={done.resume(with:$0)}
            let picker=UIDocumentPickerViewController(forOpeningContentTypes:types,asCopy:true);picker.delegate=self;presenter.present(picker,animated:true)
        }
    }
    func stage(name:String,data:Data)throws->[String:Any] {
        guard nd_size(Int64(data.count),limit) else {throw NativeFailure.refused("The file is empty or exceeds the upload limit.")}
        held.removeAll();let token=UUID().uuidString;held[token]=data
        return ["token":token,"name":name,"bytes":data.count,"cancelled":false]
    }
    func data(_ token:String)->Data? {held[token]}
    func release() {held.removeAll()}
    func share(_ presenter:UIViewController,name:String,data:Data)async throws->Bool {
        guard presenter.presentedViewController==nil else {throw NativeFailure.refused("Close the current panel.")}
        let folder=NativeWorldStorage.directory("exports/\(UUID().uuidString)")
        try FileManager.default.createDirectory(at:folder,withIntermediateDirectories:true,attributes:[.posixPermissions:0o700])
        defer {try? FileManager.default.removeItem(at:folder)}
        let file=folder.appendingPathComponent(name)
        try data.write(to:file,options:[.atomic,.completeFileProtectionUntilFirstUserAuthentication]);try FileManager.default.setAttributes([.posixPermissions:0o600],ofItemAtPath:file.path)
        return await withCheckedContinuation {done in
            let sheet=UIActivityViewController(activityItems:[file],applicationActivities:nil)
            sheet.completionWithItemsHandler={_,completed,_,_ in done.resume(returning:completed)}
            if let popover=sheet.popoverPresentationController {popover.sourceView=presenter.view;popover.sourceRect=CGRect(x:presenter.view.bounds.midX,y:presenter.view.bounds.midY,width:1,height:1)}
            presenter.present(sheet,animated:true)
        }
    }
    static func put(_ data:Data,url:String,session:URLSession)async throws->Int {
        guard url.withCString({nd_upload(coax_string(data:$0,count:Int64(url.utf8.count)))}),let address=URL(string:url) else {throw NativeFailure.refused("The upload address was refused.")}
        var request=URLRequest(url:address);request.httpMethod="PUT"
        let (_,response)=try await session.upload(for:request,from:data)
        return (response as? HTTPURLResponse)?.statusCode ?? 0
    }
    func put(token:String,url:String)async throws->[String:Any] {
        guard let bytes=held[token] else {throw NativeFailure.refused("Choose the file again before retrying its upload.")}
        defer {held[token]=nil}
        let status=try await Self.put(bytes,url:url,session:upload.session)
        return ["status":status]
    }
    func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentsAt urls:[URL]) {
        let done=completion;completion=nil
        do {
            guard let url=urls.first else {done?(.success(["cancelled":true]));return}
            let accessing=url.startAccessingSecurityScopedResource();defer {if accessing {url.stopAccessingSecurityScopedResource()}}
            let size=try url.resourceValues(forKeys:[.fileSizeKey]).fileSize ?? 0
            guard nd_size(Int64(size),limit) else {throw NativeFailure.refused("The file is empty or exceeds the upload limit.")}
            done?(.success(try stage(name:url.lastPathComponent,data:Data(contentsOf:url,options:.mappedIfSafe))))
        }catch {done?(.failure(error))}
    }
    func documentPickerWasCancelled(_ controller:UIDocumentPickerViewController) {let done=completion;completion=nil;done?(.success(["cancelled":true]))}
}
