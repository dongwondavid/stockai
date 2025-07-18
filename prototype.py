
class time:
    def __init__(self):
        pass

class real_api:
    def __init__(self):
        pass

class paper_api:
    def __init__(self):
        pass

class db_api:
    def __init__(self):
        pass

class model:
    def __init__(self):
        pass

class broker:
    def __init__(self):
        pass

class db_manager:
    def __init__(self):
        pass



class runner:
    def __init__(self):
        self.type = "real" # "real" or "paper" or "backtest"

        self.real_api = real_api() if self.type == "real" else db_api()
        self.paper_api = paper_api() if self.type == "paper" else db_api()
        self.db_api = db_api()

        self.broker_api = real_api() if self.type == "real" else paper_api() if self.type == "paper" else db_api()

        self.time = time(self.type)
        self.model = model(self.real_api, self.paper_api, self.db_api)
        self.broker = broker(self.broker_api)
        self.db_manager = db_manager()

        self.stop_condition = False
    
    def run(self):
        # on start
        self.time.on_start()
        self.model.on_start()
        self.db_manager.on_start()
        self.broker.on_start()


        while not self.stop_condition:
            # get next event and wait until next event
            self.time.update()
            wait_until_next_event(self.time)

            # model on event
            result =self.model.on_event(self.time)

            # broker on event
            if result is not None:
                broker_result = self.broker.on_event(result)
                if broker_result is not None:
                    self.db_manager.on_event(broker_result)
            
        # on end
        self.model.on_end()
        self.db_manager.on_end()
        self.broker.on_end()



def wait_until_next_event(time: time):
    pass
