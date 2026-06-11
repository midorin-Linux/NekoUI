import './App.css'
import {Routes, Route, useLocation} from "react-router-dom";
import {SidebarProvider} from "@/components/ui/sidebar.tsx";
import ChatSidebar from "@/components/chat-sidebar.tsx";
import MessageBox from "@/components/message-box.tsx"

function App() {
  const location = useLocation()
  const isHomePage = location.pathname === "/"

  const messageBoxWrapperClassName = `absolute w-full flex flex-col justify-center px-8 ${
    isHomePage ? "inset-0 items-center" : "bottom-8"
  }`

  return (
    <>
      <SidebarProvider>
        <ChatSidebar />
        <main className="w-full bg-white relative h-screen max-w-4xl mx-auto flex flex-col">
          <div className={messageBoxWrapperClassName}>
            <div className={`${ isHomePage ? "" : "hidden"}`}>
              <h1 className="scroll-m-20 text-center text-5xl tracking-tight text-balance mb-10">Hello!</h1>
            </div>
            <MessageBox />
          </div>
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
