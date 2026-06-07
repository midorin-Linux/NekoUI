import './App.css'
import {Routes, Route} from "react-router-dom";
import {SidebarProvider, SidebarTrigger} from "@/components/ui/sidebar.tsx";
import ChatSidebar from "@/components/chat-sidebar.tsx";

function App() {
  return (
    <>
      <SidebarProvider>
        <ChatSidebar />
        <main>
          <SidebarTrigger size={"lg"} className="m-1" />
          <Routes>
            <Route path="/" />
            <Route path="/sessions/:sessionId" />
          </Routes>
        </main>
      </SidebarProvider>
    </>
  )
}

export default App
