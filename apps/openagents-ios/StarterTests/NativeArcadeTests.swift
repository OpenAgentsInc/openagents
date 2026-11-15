// Firm reason: 6 (UIKit/Metal carrier oracle with the real shared component scene).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeArcadeTests:XCTestCase {
    func testDrawerMorphUsesVisibleInkFromTheSkin() {
        let button=NativeButton(.icon);button.show("Navigation",symbol:"line.3.horizontal");button.drawerProgress=1
        let paint=button.subviews.compactMap {($0 as? NativeArcadeNode)?.arcadePaint}.first {$0.kind==7}
        XCTAssertNotNil(paint);XCTAssertNotEqual((paint?.ink ?? 0)&255,0)
    }
    func testMultilineNoticeMeasuresTheSameInsetWidthItDraws() {
        let words="This is a long notice that must remain readable when the phone is narrow and the text size is large."
        let notice=NativeNotice(),label=UILabel();notice.numberOfLines=0;label.numberOfLines=0
        notice.font=UIFont.systemFont(ofSize:28);label.font=notice.font;notice.text=words;label.text=words
        let expected=label.sizeThatFits(CGSize(width:176,height:1000))
        XCTAssertEqual(notice.sizeThatFits(CGSize(width:200,height:1000)).height,expected.height+20,accuracy:1)
    }
    func testUpdateOfferUsesTheSameComponentSurface() throws {
        let home=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:home)}
        let host=NativeHost(updates:try NativeUpdates(home:home)),window=UIWindow(frame:CGRect(x:0,y:0,width:402,height:874))
        window.rootViewController=host;window.makeKeyAndVisible()
        defer {host.health?.cancel();window.isHidden=true}
        host.offerActivation()
        let prompt=try XCTUnwrap(host.presentedViewController as? NativeControls)
        XCTAssertNotNil(prompt.controls["updates.apply"] as? NativeButton)
        XCTAssertNotNil(prompt.controls["updates.ready"] as? NativeNotice)
        XCTAssertTrue(prompt.view.subviews.contains {$0 is NativeArcadeSurface})
        host.dismiss(animated:false)
    }
    func testMeasuredNativeAdaptersArePaintedByTheSharedSceneAndRetireWhenHidden() {
        let controller=UIViewController(),window=UIWindow(frame:CGRect(x:0,y:0,width:402,height:874))
        window.rootViewController=controller;window.makeKeyAndVisible();defer {window.isHidden=true}
        let root=controller.view!;root.frame=window.bounds
        let surface=NativeArcadeSurface(in:root)
        let button=NativeButton(.plain);button.show("Continue");button.frame=CGRect(x:20,y:120,width:200,height:44);root.addSubview(button);button.layoutIfNeeded()
        let field=NativeField();field.frame=CGRect(x:20,y:180,width:200,height:44);field.textContentType = .oneTimeCode;field.text="123456";root.addSubview(field);field.layoutIfNeeded()
        let clip=UIView(frame:CGRect(x:20,y:260,width:200,height:20));clip.clipsToBounds=true;root.addSubview(clip)
        let hidden=NativeFrameView();hidden.frame=CGRect(x:0,y:40,width:100,height:50);clip.addSubview(hidden)
        surface.draw();XCTAssertGreaterThanOrEqual(surface.painted,2);XCTAssertEqual(surface.refused,0)
        let count=surface.painted;button.isHidden=true;surface.draw();XCTAssertLessThan(surface.painted,count)
        XCTAssertEqual(field.text,"123456");XCTAssertEqual(field.textContentType,.oneTimeCode)
        XCTAssertFalse(root.subviews.compactMap {$0 as? NativeFrameView}.contains {$0.layer.sublayers?.contains {$0 is CAShapeLayer} == true})
    }
}
