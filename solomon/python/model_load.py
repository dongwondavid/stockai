from joblib import load

model = load("solomon\python\day1_day2_day3_day4_model.joblib")
print("model.classes_:", model.classes_)
