// Firm reason: 6 (JavaScriptCore, URLSession WebSocket and UIKit integration oracle).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeFeedTests:XCTestCase {
    private func wire(_ action:String,slot:Int=0,generation:Int=1,sequence:Int=0,payload:String="")throws->String {
        String(decoding:try JSONSerialization.data(withJSONObject:["schema":"coax.native.relay-read.v1","action":action,"slot":slot,"generation":generation,"sequence":sequence,"payload":payload]),as:UTF8.self)
    }
    func testInstalledReadCapabilityRefusesWritesAndUnknownFields()async throws {
        let service=NativeRelayServices();defer {service.reset()}
        for request in [try wire("open",payload:"ws://relay.example"),try wire("send",payload:"[\"EVENT\",{}]"),try wire("send",payload:"[\"AUTH\",{}]"),try wire("poll",generation:0)] {
            do {_=try await service.perform(request);XCTFail("Invalid read request was accepted")}catch {}
        }
        let valid=try wire("clock"),unknown=String(valid.dropLast())+",\"private_key\":\"refused\"}"
        do {_=try await service.perform(unknown);XCTFail("Unknown field was accepted")}catch {}
        let result=try await service.perform(valid),raw=try XCTUnwrap(result["record"] as? String),record=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(raw.utf8)) as? [String:Any])
        XCTAssertEqual(record["kind"] as? String,"clock");XCTAssertGreaterThan(Int64(record["payload"] as? String ?? "") ?? 0,1_700_000_000)
    }
    func testNativeImagesRetainTheExistingViewDuringFeedUpdates()throws {
        let controls=NativeControls();controls.view.frame=CGRect(x:0,y:0,width:402,height:874)
        let image=NativeElement(id:"feed.picture.0",role:"image",label:"Author picture")
        controls.render([image]);let first=try XCTUnwrap(controls.controls[image.id] as? NativeRemoteImage)
        controls.render([image,NativeElement(id:"feed.new",role:"button",label:"New notes")])
        XCTAssertTrue(controls.controls[image.id] === first)
    }
    func testReadingAnchorSurvivesNewControlsAndExplicitScopeResetsToTop()throws {
        let surface=NativeControls(),window=UIWindow(frame:CGRect(x:0,y:0,width:402,height:874));window.rootViewController=surface;window.makeKeyAndVisible();defer {window.isHidden=true}
        let marker=NativeElement(id:"feed.list",role:"reading-list",label:"",value:"1.0.0")
        let rows=(0..<35).map {NativeElement(id:"note.\($0)",role:"text",label:String(repeating:"A readable note. ",count:5))}
        surface.render([marker]+rows);surface.view.layoutIfNeeded()
        let scroll=try XCTUnwrap(surface.view.subviews.compactMap {$0 as? UIScrollView}.first)
        scroll.setContentOffset(CGPoint(x:0,y:350),animated:false)
        let kept=try XCTUnwrap(surface.controls["note.6"]),before=kept.convert(kept.bounds,to:surface.view).minY
        surface.render([marker,NativeElement(id:"feed.new",role:"button",label:"New notes ↑")]+rows);surface.view.layoutIfNeeded()
        XCTAssertEqual(kept.convert(kept.bounds,to:surface.view).minY,before,accuracy:1)
        surface.render([NativeElement(id:"feed.list",role:"reading-list",label:"",value:"2.0.0")]+rows)
        XCTAssertEqual(scroll.contentOffset.y,0,accuracy:1)
    }
    func testLiveSelectedRelaysReachTheActualPortableFeed()async throws {
        let urls=["wss://relay.primal.net","wss://nos.lol"]
        let settings:[String:Any]=["schema":"openagents.account.settings.v1","revision":0,"profile":["display_name":"","name":"","nip05":"","about":"","picture":"","banner":"","website":""],"relays":urls,"privacy":["remote_images":false,"private_clipboard":true],"notifications":["security":true]]
        let send:AppAuth.Send={ request in
            let path=request.url?.path ?? "",value:Any
            if path=="/api/auth/settings/v1" {value=settings}
            else if path=="/api/auth/identity/v1" {value=["pubkey":String(repeating:"a",count:64),"source":"account"]}
            else if path=="/api/auth/account" {value=["account":"fixture","primary":"fixture@example.com"]}
            else if path=="/api/auth/ways" {value=["level":"L1","keys":[],"passkeys":[]] as [String:Any]}
            else if path=="/api/auth/sessions" {value=["sessions":[]] as [String:Any]}
            else if path=="/api/chat/history/v1" {value=["schema":"openagents.chat.history.v1","chats":[]] as [String:Any]}
            else if path.contains("environment") {value=["schema":"openagents.environment.catalog.v1alpha1","environments":[]] as [String:Any]}
            else {return (Data("{}".utf8),404)}
            return (try JSONSerialization.data(withJSONObject:value),200)
        }
        let auth=AppAuth(origin:"https://fixture.example",send:send,store:AccountTokenOracle(),device:nil)
        let home=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:home)}
        let host=NativeHost(updates:try NativeUpdates(home:home),auth:auth);host.accountServices.send=send
        let p=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
        var opened=Set<Int>(),events=Set<Int>(),requests=0,generations:[Int:Int]=[:]
        func drain()async throws {
            let names=[1:"auth.state",4:"http",8:"http.cancel",11:"environment.commandIDs",12:"environment.wait",18:"account.service",20:"relay.read"]
            for _ in 0..<64 {
                let (op,body)=try p.action();if op==0 {return}
                let name=try XCTUnwrap(names[op]);var args=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(body.utf8)) as? [String:Any]);if op==20 {args["nativeWire"]=body;requests+=1}
                do {
                    let response=try await host.call(name,args),result=try XCTUnwrap(response as? [String:Any]);let raw:String
                    if op==4 {raw=result["body"] as? String ?? ""}
                    else if op==18 || op==20 {raw=result["record"] as? String ?? ""}
                    else {raw=String(decoding:try JSONSerialization.data(withJSONObject:result),as:UTF8.self)}
                    if op==20,let observation=(try? JSONSerialization.jsonObject(with:Data(raw.utf8))) as? [String:Any],let slot=observation["slot"] as? Int {
                        if observation["kind"] as? String=="open" {opened.insert(slot);generations[slot]=observation["generation"] as? Int}
                        if observation["kind"] as? String=="message",let text=observation["payload"] as? String,let frame=(try? JSONSerialization.jsonObject(with:Data(text.utf8))) as? [Any],frame.first as? String=="EVENT",frame.count==3,let event=frame[2] as? [String:Any],event["kind"] as? Int==1 {events.insert(slot)}
                    }
                    try p.event(100+op,result["status"] as? Int ?? 0,raw)
                }catch {try p.event(200+op,0,"{}")}
                try p.frame()
            }
            XCTFail("Read carrier did not yield within its bounded drain")
        }
        try p.event(Int(cnw_enable_event()),0,"");try p.frame();try await drain()
        try p.input("nav.toggle");try p.frame();try await drain();try p.input("nav.feed");try p.frame();try await drain()
        let deadline=Date().addingTimeInterval(30)
        while Date()<deadline && (opened.count<2 || events.count<2 || !p.nodes.values.contains {$0.id=="feed.note.0"}) {
            try await Task.sleep(for:.milliseconds(100));try p.frame(elapsed:0.1);try await drain()
        }
        XCTAssertEqual(opened,Set([0,1]));XCTAssertEqual(events,Set([0,1]));XCTAssertTrue(p.nodes.values.contains {$0.id=="feed.note.0"})
        XCTAssertFalse(p.nodes.values.contains {$0.id=="feed.refresh" || $0.id=="chat.composer"})
        let surface=NativeControls(),workspace=NativeWorkspace(detail:surface),window=UIWindow(frame:CGRect(x:0,y:0,width:402,height:874));window.rootViewController=workspace;window.makeKeyAndVisible()
        defer {window.isHidden=true}
        workspace.render(p.elements(),history:p.elements(navigation:true));workspace.view.layoutIfNeeded()
        let shot=XCTAttachment(image:UIGraphicsImageRenderer(bounds:workspace.view.bounds).image {_ in workspace.view.drawHierarchy(in:workspace.view.bounds,afterScreenUpdates:true)})
        shot.name="Native kind-1 feed";shot.lifetime = .keepAlways;add(shot)
        let receipt:[String:Any]=["selected_relays":urls,"opened_slots":opened.sorted(),"kind1_slots":events.sorted(),"carrier_requests":requests,"native_note_present":p.nodes.values.contains {$0.id=="feed.note.0"},"remote_images":false]
        let evidence=XCTAttachment(data:try JSONSerialization.data(withJSONObject:receipt,options:.prettyPrinted),uniformTypeIdentifier:"public.json");evidence.name="Native feed relay receipts";evidence.lifetime = .keepAlways;add(evidence)
        // Exercise the SDK lifecycle on the physical device. Inject its actual
        // notification, then advance the portable retry clock independently of
        // wall time; neither a new identity nor a second socket stack is used.
        let previous=generations
        NotificationCenter.default.post(name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.post(name:UIApplication.didBecomeActiveNotification,object:nil)
        let reconnectDeadline=Date().addingTimeInterval(20)
        while Date()<reconnectDeadline && ![0,1].allSatisfy({(generations[$0] ?? 0)>(previous[$0] ?? 0)}) {
            try await Task.sleep(for:.milliseconds(100));try p.frame(elapsed:0.5);try await drain()
        }
        XCTAssertTrue([0,1].allSatisfy {(generations[$0] ?? 0)>(previous[$0] ?? 0)},"Foreground must open fresh owned generations on both selected relays")
        XCTAssertTrue(p.nodes.values.contains {$0.id.hasPrefix("feed.note.")})
        _=try await host.call("relay.read",["nativeWire":wire("reset",generation:0)])
    }
}
