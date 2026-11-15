// Firm reason: 6 (bounded URLSession and ImageIO decoding for the shared image node).
import UIKit
import ImageIO

@MainActor final class NativeRemoteImage:UIImageView {
    private var source="",task:Task<Void,Never>?
    private let transport=NativeWorldSession()
    private var height:NSLayoutConstraint!
    init() {
        super.init(frame:.zero);contentMode = .scaleAspectFill;clipsToBounds=true
        isAccessibilityElement=true;height=heightAnchor.constraint(equalToConstant:CGFloat(NativeSkin.tokens.profile_picture));height.isActive=true
    }
    required init?(coder:NSCoder) {fatalError()}
    deinit {task?.cancel();transport.close()}
    func render(_ e:NativeElement) {
        height.constant=CGFloat(e.hasClass("img-big") ? NativeSkin.tokens.profile_banner:NativeSkin.tokens.profile_picture);accessibilityLabel=e.attrs["aria-label"]
        let next=e.attrs["src"] ?? "";guard next != source else{return};source=next;task?.cancel();image=nil
        // The Coax kit has already admitted the URL; no account headers/cookies are attached.
        guard NativeAccountCodec.text(next,{nas_image($0)}),let url=URL(string:next) else{return}
        task=Task { [weak self,transport] in
            do {
                let (data,status)=try await transport.fetch(URLRequest(url:url,timeoutInterval:15),limit:2097152)
                guard !Task.isCancelled,status==200,let cg=CGImageSourceCreateWithData(data as CFData,nil),
                      let thumb=CGImageSourceCreateThumbnailAtIndex(cg,0,[kCGImageSourceCreateThumbnailFromImageAlways:true,kCGImageSourceThumbnailMaxPixelSize:1024,kCGImageSourceCreateThumbnailWithTransform:true] as CFDictionary) else{return}
                self?.image=UIImage(cgImage:thumb)
            }catch { /* A missing picture never blocks the account screen. */ }
        }
    }
}
